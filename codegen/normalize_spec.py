#!/usr/bin/env python3
"""Turn Truto's openapi.json into a small, language-neutral model IR.

The IR is what the per-language model generators read. It flattens the parts of
OpenAPI that are awkward to emit (allOf, inline objects, inline enums) into a
flat list of named types, and names the inline request/response bodies of each
operation so every SDK uses the same type names.

Usage: python3 normalize_spec.py openapi.json > models.json

IR shape:
  {
    "models": [
      {"name", "description", "kind": "object", "fields": [Field...], "additional": TypeRef|null}
      {"name", "description", "kind": "enum", "values": ["a", "b"]}
      {"name", "description", "kind": "union", "variants": ["ModelA", "ModelB"], "discriminator": "format"|null}
    ],
    "operations": [
      {"method", "path", "request": TypeRef|null, "response": TypeRef|null,
       "list_item": TypeRef|null}    # set when the response is a cursor page
    ]
  }
  Field   = {"name", "type": TypeRef, "required": bool, "nullable": bool, "description"}
  TypeRef = {"kind": "string"|"integer"|"number"|"boolean"|"any"}
          | {"kind": "string", "format": "date-time"|...}
          | {"kind": "ref", "name": "Model"}
          | {"kind": "array", "items": TypeRef}
          | {"kind": "map", "values": TypeRef}
"""
import json
import re
import sys

# Dashboard-only routes. They need a signed-in user session, not an API token,
# so they are left out of the SDKs.
EXCLUDED_PREFIXES = (
    "/auth/",
    "/billing/",
    "/assistant/",
    "/link-ui/",
    "/platform/",
    "/sso-connection",
    "/scim-token",
    "/scim-group",
    "/group-mapping",
)

# Path segments whose plural spelling would give a different type name from
# the rest of the resource (GET /integration vs GET /integrations/{id}).
PATH_ALIASES = {"integrations": "integration"}

PRIMITIVES = {"string", "integer", "number", "boolean"}


def pascal(s):
    parts = re.split(r"[^A-Za-z0-9]+", s)
    out = ""
    for p in parts:
        if not p:
            continue
        out += p[0].upper() + p[1:]
    if out and out[0].isdigit():
        out = "N" + out
    return out


class Normalizer:
    def __init__(self, spec):
        self.spec = spec
        self.schemas = spec.get("components", {}).get("schemas", {})
        self.models = {}  # name -> model
        self.order = []
        # Components that are not a named model (an inline oneOf of primitives,
        # an array, ...) are inlined wherever they are referenced.
        self.aliases = {}  # component name -> TypeRef
        self.in_progress = set()

    # -- helpers -----------------------------------------------------------

    def resolve(self, schema):
        seen = 0
        while "$ref" in schema:
            name = schema["$ref"].split("/")[-1]
            schema = self.schemas[name]
            seen += 1
            if seen > 20:
                raise ValueError("ref loop")
        return schema

    def add(self, model):
        name = model["name"]
        if name in self.models:
            existing = self.models[name]
            if existing == model:
                return name
            # Same name, different shape: pick a free suffix.
            i = 2
            while f"{name}{i}" in self.models:
                i += 1
            model = dict(model, name=f"{name}{i}")
            name = model["name"]
        self.models[name] = model
        self.order.append(name)
        return name

    @staticmethod
    def is_object_like(s):
        return s.get("type") == "object" or "properties" in s or "allOf" in s

    # -- type conversion ---------------------------------------------------

    def type_of(self, schema, hint):
        """Return a TypeRef for `schema`. `hint` names any inline type created."""
        if schema is None or schema is True or schema == {}:
            return {"kind": "any"}
        if "$ref" in schema:
            name = schema["$ref"].split("/")[-1]
            if name not in self.models and name not in self.aliases and name not in self.in_progress:
                self.component(name, self.schemas[name])
            if name in self.aliases:
                return self.aliases[name]
            return {"kind": "ref", "name": name}

        if "allOf" in schema:
            parts = schema["allOf"]
            # allOf with one $ref (often used to attach a description) is just that ref.
            if len(parts) == 1:
                return self.type_of(parts[0], hint)
            return {"kind": "ref", "name": self.object_model(hint, schema)}

        if "oneOf" in schema or "anyOf" in schema:
            variants = schema.get("oneOf") or schema.get("anyOf")
            non_null = [v for v in variants if v.get("type") != "null"]
            if len(non_null) == 1:
                return self.type_of(non_null[0], hint)
            # A union of primitives or inline shapes has no useful static type.
            if all("$ref" in v for v in non_null):
                refs = [v["$ref"].split("/")[-1] for v in non_null]
                if all(self.is_object_like(self.resolve(v)) for v in non_null):
                    return {"kind": "ref", "name": self.union_model(hint, schema, refs)}
            return {"kind": "any"}

        t = schema.get("type")
        if isinstance(t, list):
            t = next((x for x in t if x != "null"), None)

        if t == "string" and "enum" in schema:
            values = [v for v in schema["enum"] if isinstance(v, str)]
            if len(values) == len(schema["enum"]) and values:
                return {"kind": "ref", "name": self.add({
                    "name": hint, "kind": "enum",
                    "description": schema.get("description", ""), "values": values,
                })}
            return {"kind": "string"}

        if t in PRIMITIVES:
            ref = {"kind": t}
            if t == "string" and schema.get("format") in ("date-time", "date", "uuid", "binary"):
                ref["format"] = schema["format"]
            return ref

        if t == "array":
            return {"kind": "array", "items": self.type_of(schema.get("items"), hint + "Item")}

        if t == "object" or "properties" in schema:
            if schema.get("properties"):
                return {"kind": "ref", "name": self.object_model(hint, schema)}
            addl = schema.get("additionalProperties")
            if isinstance(addl, dict) and addl:
                return {"kind": "map", "values": self.type_of(addl, hint + "Value")}
            return {"kind": "map", "values": {"kind": "any"}}

        return {"kind": "any"}

    def collect_object(self, schema, owner):
        """Merge an object schema (resolving allOf) into (props, required, additional).

        props maps each property to (schema, owner): owner is the name of the
        model that declared it, so an inline enum inherited through allOf is
        named after (and shared with) the model that declared it.
        """
        if "$ref" in schema:
            owner = schema["$ref"].split("/")[-1]
        schema = self.resolve(schema)
        props, required, additional = {}, set(), None
        for part in schema.get("allOf", []):
            p, r, a = self.collect_object(part, owner)
            props.update(p)
            required |= r
            additional = additional or a
        for k, v in schema.get("properties", {}).items():
            props[k] = (v, owner)
        required |= set(schema.get("required", []))
        if isinstance(schema.get("additionalProperties"), dict) and schema["additionalProperties"]:
            additional = schema["additionalProperties"]
        return props, required, additional

    def object_model(self, name, schema):
        props, required, additional = self.collect_object(schema, name)
        fields = []
        for pname, (pschema, owner) in props.items():
            nullable = bool(pschema.get("nullable")) or (
                isinstance(pschema.get("type"), list) and "null" in pschema["type"]
            )
            desc = pschema.get("description", "")
            fields.append({
                "name": pname,
                "type": self.type_of(pschema, owner + pascal(pname)),
                "required": pname in required,
                "nullable": nullable,
                "description": desc.strip(),
            })
        model = {
            "name": name, "kind": "object",
            "description": (schema.get("description") or "").strip(),
            "fields": fields,
            "additional": self.type_of(additional, name + "Value") if additional else None,
        }
        return self.add(model)

    def union_model(self, name, schema, refs):
        disc = schema.get("discriminator", {}).get("propertyName")
        return self.add({
            "name": name, "kind": "union",
            "description": (schema.get("description") or "").strip(),
            "variants": refs, "discriminator": disc,
        })

    # -- entry points ------------------------------------------------------

    def run(self):
        # Components first, in spec order, so their names win.
        for name, schema in self.schemas.items():
            self.component(name, schema)

        ops = []
        for path, item in self.spec["paths"].items():
            if path.startswith(EXCLUDED_PREFIXES):
                continue
            for method, op in item.items():
                if method not in ("get", "post", "put", "patch", "delete"):
                    continue
                ops.append(self.operation(path, method, op, item))

        # Drop component models no operation or model reaches? No: keep every
        # component, SDK users may want them for webhooks and payloads.
        return {
            "models": [self.models[n] for n in self.order],
            "operations": ops,
        }

    def component(self, name, schema):
        if name in self.models or name in self.aliases or name in self.in_progress:
            return
        self.in_progress.add(name)
        try:
            self._component(name, schema)
        finally:
            self.in_progress.discard(name)

    def _component(self, name, schema):
        t = self.type_of(schema, name)
        if t["kind"] == "ref" and t["name"] == name:
            return
        if t["kind"] == "ref" and t["name"] != name:
            # A component that is just an alias of another: emit an object copy
            # so the component name exists in every SDK.
            target = self.models.get(t["name"]) or self.resolve({"$ref": "#/components/schemas/" + t["name"]})
            if isinstance(target, dict) and target.get("kind"):
                self.add(dict(target, name=name))
            return
        # A map component stays a named (field-less) object model; anything
        # else (any, array, primitive) is inlined where it is referenced.
        if t["kind"] == "map":
            self.add({"name": name, "kind": "object",
                      "description": (schema.get("description") or "").strip(),
                      "fields": [], "additional": t["values"]})
        else:
            self.aliases[name] = t

    def op_base(self, path, method, item):
        segs = [s for s in path.strip("/").split("/")]
        literals = [PATH_ALIASES.get(s, s) for s in segs if not s.startswith("{")]
        base = pascal("-".join(literals))
        last_is_param = segs[-1].startswith("{")
        if last_is_param:
            verb = {"get": "Get", "patch": "Update", "put": "Replace", "delete": "Delete", "post": "Post"}[method]
        else:
            has_get = "get" in item
            if method == "get":
                verb = "List" if has_get and "post" in item else ""
            elif method == "post":
                verb = "Create" if has_get else ""
            elif method == "delete":
                verb = "Delete"
            elif method == "patch":
                verb = "Update"
            else:
                verb = "Replace"
        return base + verb

    def operation(self, path, method, op, item):
        base = self.op_base(path, method, item)

        req = None
        body = op.get("requestBody", {}).get("content", {})
        bschema = (body.get("application/json") or {}).get("schema")
        if bschema:
            req = self.type_of(bschema, base + "Request")

        resp, list_item = None, None
        responses = op.get("responses", {})
        ok = next((v for k, v in sorted(responses.items()) if k.startswith("2")), None)
        if ok:
            rschema = ((ok.get("content") or {}).get("application/json") or {}).get("schema")
            if rschema:
                r = self.resolve(rschema) if "$ref" not in rschema else self.resolve(rschema)
                props = r.get("properties", {}) if isinstance(r, dict) else {}
                arr_key = next((k for k in ("result", "results") if k in props and props[k].get("type") == "array"), None)
                if method == "get" and arr_key and "next_cursor" in props:
                    list_item = self.type_of(props[arr_key].get("items"), base + "Item")
                else:
                    resp = self.type_of(rschema, base + "Response")

        return {"method": method.upper(), "path": path, "request": req,
                "response": resp, "list_item": list_item}


def main():
    spec = json.load(open(sys.argv[1]))
    ir = Normalizer(spec).run()
    json.dump(ir, sys.stdout, indent=1)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main()

import json

with open("rolx-openapi.json", "r") as f:
    spec = json.load(f)

for path, path_item in spec.get("paths", {}).items():
    if not isinstance(path_item, dict):
        continue

    for method in ["get", "post", "put", "delete", "patch"]:
        if method in path_item and isinstance(path_item[method], dict):
            op = path_item[method]

            # 1. Ensure operationId exists
            if "operationId" not in op:
                clean_path = path.strip("/").replace("/", "_").replace("-", "_").replace("{", "").replace("}", "")
                op["operationId"] = f"{method}_{clean_path}"

            # 2. Flatten requestBody to a single media type
            if "requestBody" in op and isinstance(op["requestBody"], dict):
                content = op["requestBody"].get("content")
                if isinstance(content, dict) and len(content) > 1:
                    # Keep application/json if present, otherwise take the first available
                    chosen_media = "application/json" if "application/json" in content else next(iter(content))
                    op["requestBody"]["content"] = {
                        chosen_media: content[chosen_media]
                    }

            # 3. Flatten responses to a single media type per status code (optional guard)
            if "responses" in op and isinstance(op["responses"], dict):
                for status_code, resp in op["responses"].items():
                    if isinstance(resp, dict) and "content" in resp and isinstance(resp["content"], dict):
                        resp_content = resp["content"]
                        if len(resp_content) > 1:
                            chosen_media = "application/json" if "application/json" in resp_content else next(iter(resp_content))
                            resp["content"] = {
                                chosen_media: resp_content[chosen_media]
                            }
                

with open("rolx-openapi.with-operation-id.json", "w") as f:
    json.dump(spec, f, indent=2)
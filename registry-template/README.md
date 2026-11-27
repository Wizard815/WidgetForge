# WidgetForge widgets (registry template)

PLACEHOLDER wording. This folder is a ready-made repository for the community widget
catalog that the WidgetForge Hub's **Browse** page reads. It is not published yet: create a
GitHub repository, copy this folder's contents into it, and turn on GitHub Pages
(Settings → Pages → Source: GitHub Actions).

The catalog address is then `https://<owner>.github.io/<repo>/index.json`. Put that address
in the app (Hub → Browse → Catalog address), or set it as the app's default in
`src-tauri/src/registry.rs` (`DEFAULT_INDEX`).

## Adding a widget

1. Fork this repository.
2. Add your widget as `widgets/<your.widget-id>/` with its `widget.yaml` (and `icon.svg`,
   assets, `index.html` for HTML widgets). The folder name must be the widget's id.
3. Fill in `name`, `version` (like 1.0.0), `author` and `description`.
4. Open a pull request. A check lists the permissions your widget asks for and stops anything
   the app would refuse (see below). A maintainer reviews it.
5. To update, raise `version` and open another pull request. People who installed it see
   "Update" in Browse, with any new permissions marked.

## What the check refuses

The same things the app refuses when installing:

- ids starting with `forge.` (the app's own widgets), or a folder not named after the id;
- links, files over 20 MB, more than 100 MB or 2000 files in total;
- a `command` setting with a default (only the user may type a command to run);
- unknown permissions, and network permissions with paths or wildcards.

Run it yourself: `pip install pyyaml && python3 tools/build_index.py`.

## index.json

```json
{ "format": "widgetforge-index", "version": 1, "generated": "…",
  "widgets": [ { "id", "name", "version", "author", "description", "category",
                 "permissions", "html", "download", "sha256", "size", "icon", "homepage" } ] }
```

`download` and `icon` are relative to the index address. The app checks each download's
size and SHA-256 against the index before it shows the install screen.

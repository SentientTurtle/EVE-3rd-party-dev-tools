# Supplemental documentation for `web_dir` output

Web_dir output writes two sets of files:
* `{type_id}.json`, a list of available icon types (e.g. `['icon', 'render'])
* `{type_id}_{icon_type}.png` (or `.jpg`), the icon images themselves.
Depending on the setting, these may be symlinks into the tool's icon cache, or copied 'real' files.
These should be mapped onto the routes:
* `/{type_id}/` -> `{type_id}.json`
* `/{type_id}/{icon_type}` -> `{type_id}_{icon_type}.png` (or `.jpg`)

JPGs are only used for renders, so another valid mapping is:
* `/{type_id}` -> `{type_id}.json`
* `/{type_id}/render` -> `{type_id}_render.jpg`
* `/{type_id}/{icon_type not 'render'}` -> `{type_id}_{icon_type}.png`

Example nginx server block:
```
disable_symlinks off;
location = / {
    try_files /index.html =404;
}
location ~ ^/types/(\d+)/?$ {
    try_files /$1.json =404;
}
location ~ ^/types/(\d+)/([a-z]+)/?$ {
    try_files "/$1_$2.png" "/$1_$2.jpg" =404;
}
location /alliances {
    return 301 $scheme://images.evetech.net$uri;
}
location /characters {
    return 301 $scheme://images.evetech.net$uri;
}
location /corporations {
    return 301 $scheme://images.evetech.net$uri;
}
```
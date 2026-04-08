# Textus

Compile-time validated folder-based templating for Rust, powered by a derive macro.

Textus walks a directory of template files at compile time, extracts `{{ var }}` placeholders,
and checks them against your struct fields — catching mismatches before your code ever runs.

## Usage

Given a `templates/` folder:

```
templates/
├── greeting.txt       →  Hello, {{ name }}!
└── config/app.toml    →  title = "{{ name }}"
```

Define a template struct:

```rust
use textus::Template;

#[derive(Template)]
#[template(path = "templates/")]
struct Page {
    name: String,
}

fn main() {
    let page = Page { name: "World".into() };
    for (path, content) in page.render() {
        println!("{path}: {content}");
    }
}
```

## Validation modes

Set the mode with `#[template(path = "...", mode = "strict")]`.

| Mode        | Template vars must exist as fields | Fields must appear in templates |
|-------------|------------------------------------|---------------------------------|
| `"default"` | ✓                                  | ✗                               |
| `"strict"`  | ✓                                  | ✓                               |
| `"lenient"` | ✗                                  | ✗                               |

## How it works

- The derive macro runs at compile time, reading and parsing every file under `path`.
- Variables (`{{ var }}`) are matched against the struct's named fields.
- Mismatches produce clear `compile_error!` messages with context.
- Templates without variables return `Cow::Borrowed` (zero allocation); dynamic ones use `format!` and return `Cow::Owned`.
- File changes are tracked via `include_bytes!`, so `cargo` rebuilds automatically when templates change.

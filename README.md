# Textus

Compile-time validated folder-based templating for Rust, powered by a derive macro.

Textus walks a directory of template files at compile time, extracts `{{ var }}` placeholders, and checks them against your struct fields — catching mismatches before your code ever runs.

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

## Validation

Validation is strict, and there is nothing to opt into:

- Every `{{ var }}` must have a matching struct field.
- Every struct field must appear in at least one template.

Either mismatch is a compile error.

## Options

| Option                 | Effect                                                    |
|------------------------|-----------------------------------------------------------|
| `path = "..."`         | Template directory, relative to the crate root. Required. |
| `literal`              | Files are copied verbatim.                                |
| `strip_prefix = "..."` | Trim a prefix from each output path.                      |
| `strip_suffix = "..."` | Trim a suffix from each output path.                      |
| `root = ...`           | Path to the `textus` crate. Defaults to `::textus`.       |

### `literal`

With `literal`, files aren't processed, so `{{ ... }}` is left exactly as written.

```rust
#[derive(Template)]
#[template(path = "assets/", literal, strip_suffix = ".tmpl")]
struct Assets;
```

```
assets/main.css.tmpl  →  main.css
```

Path rewriting still works as usual, since `strip_prefix` / `strip_suffix` act on output paths rather than contents.

## `no_std`

`textus` is `no_std`-compatible; disable default features and it needs only `alloc`:

```toml
textus = { version = "0.3", default-features = false }
```

`Template::render` is always available. `render_into`, which writes the rendered files to disk, requires the `std` feature (on by default).

### `root`

Can be used by framework authors if they re-export this library.

## How it works

- The derive macro runs at compile time, reading and parsing every file under `path`.
- Variables (`{{ var }}`) are matched against the struct's named fields.
- Mismatches produce clear `compile_error!` messages with context.
- Templates without variables are embedded with `include_str!` and returned as `Cow::Borrowed` (zero allocation); dynamic ones use `format!` and return `Cow::Owned`.
- File changes are tracked so `cargo` rebuilds automatically when templates change.

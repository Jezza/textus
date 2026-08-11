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

Both `#[template(...)]` and `#[embed(...)]` accept the same options:

| Option                 | Effect                                                    |
|------------------------|-----------------------------------------------------------|
| `path = "..."`         | Template directory, relative to the crate root. Required. |
| `strip_prefix = "..."` | Trim a prefix from each output path.                      |
| `strip_suffix = "..."` | Trim a suffix from each output path.                      |
| `root = ...`           | Path to the `textus` crate. Defaults to `::textus`.       |

## `Embed`

For files that should be copied verbatim, derive `Embed` instead. Contents are never parsed, so `{{ ... }}` is left exactly as written, and files needn't be valid UTF-8 — binary assets such as images and fonts work unchanged.

```rust
use textus::Embed;

#[derive(Embed)]
#[embed(path = "assets/", strip_suffix = ".tmpl")]
struct Assets;

for (path, bytes) in Assets::iter() {
    // path: &'static str, bytes: &'static [u8]
}
```

```
assets/main.css.tmpl  →  main.css
```

`Embed` reads no fields, so any struct shape works and none of the `Template` validation applies. Path rewriting behaves the same, since `strip_prefix` / `strip_suffix` act on output paths rather than contents.

### Migrating from `literal`

The `literal` flag was replaced by `Embed` in 0.4:

```rust
// before
#[derive(Template)]
#[template(path = "assets/", literal, strip_suffix = ".tmpl")]
struct Assets;

// after
#[derive(Embed)]
#[embed(path = "assets/", strip_suffix = ".tmpl")]
struct Assets;
```

Note that contents are now `&'static [u8]` rather than `Cow<'static, str>`, so text call sites need `str::from_utf8`.

## `no_std`

`textus` is `no_std`-compatible; disable default features and it needs only `alloc`:

```toml
textus = { version = "0.4", default-features = false }
```

`Template::render` and `Embed::iter` are always available. `Template::render_into` and `Embed::write_into`, which write files to disk, require the `std` feature (on by default).

### `root`

Can be used by framework authors if they re-export this library.

## How it works

- The derive macro runs at compile time, reading and parsing every file under `path`.
- Variables (`{{ var }}`) are matched against the struct's named fields.
- Mismatches produce clear `compile_error!` messages with context.
- Templates without variables are embedded with `include_str!` and returned as `Cow::Borrowed` (zero allocation); dynamic ones use `format!` and return `Cow::Owned`.
- `Embed` skips all of that, emitting a `&'static` slice of `include_bytes!` pairs.
- File changes are tracked so `cargo` rebuilds automatically when templates change. Adding or removing a file does not trigger a rebuild, since only file contents are tracked.

---
tags: [volgit, rust, aprendizaje]
---

# Rust en Volgit

Volver a [[Volgit]]

Mi chuleta de Rust. Voy apuntando cada cosa que aparece en el proyecto con un ejemplo sacado del propio código, que así se me queda mejor.

## Tipos que salen todo el rato

| Tipo | Qué es |
|---|---|
| `String` | Texto que es mío: soy el dueño de la memoria y lo puedo cambiar |
| `&str` | Texto prestado: una vista de un texto que es de otro |
| `Vec<T>` | Una lista que crece |
| `&[T]` | Un trozo prestado de una lista (un *slice*) |
| `u64`, `usize` | Enteros sin signo. `usize` es el de los tamaños y los índices |
| `f32`, `f64` | Decimales |
| `(A, B)` | Una tupla. Por ejemplo `(String, u64)`, nombre del lenguaje y bytes |
| `HashMap<K, V>` | Un diccionario, clave → valor |

## Propiedad y préstamos (`&`)

Esto es lo más raro de Rust viniendo de otros lenguajes: cada valor tiene un único dueño. Si una función solo necesita leer algo, lo pide prestado con `&`:

```rust
pub fn print(rep: &Report, avatar: Option<&Avatar>)   // lee el informe pero no se lo queda
```

Si además lo tiene que cambiar, necesita `&mut`:

```rust
fn round_corners(img: &mut RgbaImage, r: f32)   // cambia los píxeles de la imagen original
```

Si no pongo `&`, la función se queda con el valor y yo ya no lo puedo usar después. Por eso de vez en cuando aparece un `.clone()`: hace una copia cuando necesito que haya dos dueños.

## Option: puede haber algo o no

En Rust no existe `null`. Si algo puede faltar, es un `Option<T>`, que vale `Some(valor)` o `None`, y el compilador me obliga a tener en cuenta los dos casos.

Los métodos que más uso:

```rust
opt.unwrap_or(x)            // el valor, o x si es None
opt.unwrap_or_default()     // el valor, o el "vacío" de su tipo ("", 0, vec![])
opt.map(|v| ...)            // transforma el valor si lo hay
opt.map_or(x, |v| ...)      // lo transforma, o usa x si es None
opt.filter(|v| cond)        // lo convierte en None si no cumple la condición
cond.then(|| valor)         // de bool a Option: Some(valor) si es true, None si es false
```

Un caso real que junta tres: "la descripción, si la hay y no está vacía":

```rust
if let Some(d) = r.description.as_deref().filter(|d| !d.is_empty()) { ... }
```

`as_deref()` convierte un `Option<String>` en `Option<&str>`, o sea, presta el texto en vez de moverlo.

## Result y el operador ?

Lo que puede fallar devuelve un `Result`, que es `Ok(valor)` o `Err(error)`. El `?` significa "si es un error, devuélvelo hacia arriba y sal de la función, y si no, dame el valor":

```rust
let resp = req.send().with_context(|| format!("fallo de red en {path}"))?;
```

Para los errores uso la librería `anyhow`:
- `anyhow::Result<T>` es un `Result` que acepta cualquier tipo de error.
- `bail!("mensaje")` devuelve un error en ese mismo momento.
- `.context("...")` y `.with_context(|| ...)` le añaden una explicación al error.

Como `main` devuelve `Result<()>`, cualquier error que suba hasta arriba acaba impreso como `Error: mensaje`.

El `?` también vale con `Option`, dentro de funciones que devuelven `Option`:

```rust
let (owner, name) = (parts.next()?, parts.next()?);   // si falta alguno, return None
```

## match

Es como un switch pero mucho más potente: me obliga a cubrir todos los casos y acepta condiciones extra con `if` (los *guards*):

```rust
match (verb, title) {
    ("", t) => t.to_string(),                                   // sin verbo
    (v, "") => v.to_string(),                                   // sin título
    (v, t) if t.starts_with('#') => format!("{v} {t}"),         // guard: si es un número
    (v, t) => format!("{v}: {t}"),                              // cualquier otro caso
}
```

Ese es `with_verb`, de `render.rs`. También se puede hacer `match` sobre rangos (`d if d < 30`) y sobre tuplas de booleanos, como en `cell()`: ver [[Foto de perfil#Pintar una celda]].

## let-else

```rust
let Some((tag, detail)) = describe(e) else { continue };
```

Significa "si `describe` devuelve `Some`, saca `tag` y `detail`, y si no, haz lo del `else`". El `else` tiene que salir del bloque sí o sí, con `continue`, `return` o `break`. Me ahorra un montón de `if`s anidados.

## Closures

`|argumentos| cuerpo` es una función sin nombre. Están por todas partes:

```rust
repos.retain(|r| !r.fork);                                           // quedarme con los que no son fork
repos.sort_by(|a, b| b.stargazers_count.cmp(&a.stargazers_count));   // de más a menos estrellas
```

## Iteradores

En vez de bucles `for` con índices, en Rust se encadenan operaciones:

```rust
let total: u64 = langs.iter().map(|l| l.1).sum();
//               recorrer   → coger los bytes → sumar
```

Los que más salen: `iter`, `map`, `filter`, `filter_map` (filtra y transforma a la vez), `take(n)`, `zip` (recorre dos listas a la par), `enumerate` (me da el índice también), `collect` (lo junta todo en un `Vec` o un `String`) y `sum`.

## Structs, impl y derive

```rust
pub struct GitHub {                 // los datos
    client: Client,
    token: Option<String>,
}

impl GitHub {                       // los métodos
    pub fn new(token: Option<String>) -> Result<Self> { ... }   // el "constructor" (es solo una convención)
    fn get<T>(&self, path: &str) ... { ... }                     // &self = método de la instancia
}
```

`#[derive(...)]` escribe código por mí:
- `Debug`: poder imprimirlo con `{:?}`.
- `Deserialize`: poder construirlo desde un JSON.
- `Serialize`: poder convertirlo en JSON.
- `Parser` (de clap): poder construirlo desde los argumentos de la terminal.
- `ValueEnum` (de clap): que un enum sirva como valor de una opción. Lo uso en `ImageArg`, que así acepta `auto`, `kitty` o `blocks`.

`pub` es "visible desde otros módulos". Sin `pub`, solo se ve dentro de su archivo.

## Enums

Un enum es un tipo que puede ser una cosa u otra de una lista cerrada:

```rust
pub enum Mode {
    Kitty,
    Blocks,
}
```

Y con `match` el compilador me obliga a tratar todos los casos. Si algún día añado `Sixel`, me va a avisar en cada `match` donde se me haya olvidado.

## Genéricos y traits

```rust
fn get<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>>
```

`T` es "un tipo cualquiera", y `: DeserializeOwned` es una condición: tiene que ser un tipo que se pueda sacar de un JSON. Un *trait* viene a ser una interfaz, un conjunto de cosas que un tipo sabe hacer. Ver [[API de GitHub#La función get]].

`impl std::fmt::Display` en un parámetro quiere decir "cualquier cosa que se pueda imprimir":

```rust
fn kv(&self, label: &str, value: impl std::fmt::Display) -> String
```

Por eso a `kv` le puedo pasar un `String`, un `&str` o un texto con color.

## Módulos

Cada archivo de `src/` es un módulo. `main.rs` los declara con `mod github;` y los usa como `github::GitHub::new(...)`. Desde dentro de otro módulo, `use crate::avatar::Avatar;` trae algo de otro archivo del proyecto, y `crate::term::width()` llama a algo sin importarlo.

## Tests

```rust
#[cfg(test)]           // esto solo se compila cuando hago `cargo test`
mod tests {
    use super::*;      // trae todo lo del archivo en el que está

    #[test]
    fn usuarios() {
        assert_eq!(parse_user("@BurntSushi").as_deref(), Some("BurntSushi"));
    }
}
```

Se lanzan con `cargo test`. Tengo tests en `main.rs` (leer la entrada y `--top`) y en `render.rs` (`distribute` y `wrap`).

## unsafe y cfg

- `unsafe { ... }` quita algunas comprobaciones del compilador. Solo hace falta para cosas que Rust no puede verificar, como llamar a funciones de C. Lo uso dos veces: para lo de SIGPIPE (ver [[Arquitectura#SIGPIPE y el único unsafe de main]]) y en `term.rs`, para preguntarle a la terminal su tamaño con `ioctl`.
- `#[cfg(unix)]` compila un trozo solo en ciertos sistemas, y `#[cfg(test)]` solo en los tests.

## Las dependencias (Cargo.toml)

| Crate | Para qué la uso |
|---|---|
| `clap` | Los argumentos y el `--help` |
| `reqwest` | Las peticiones HTTP |
| `serde` / `serde_json` | Pasar de JSON a structs y al revés |
| `anyhow` | Errores cómodos |
| `colored` | Colores en la terminal |
| `chrono` | Las fechas ("hace 3 días") |
| `image` | Abrir, redimensionar y guardar la foto |
| `base64` | Codificar la foto para mandársela a kitty |
| `libc` | Hablar con el sistema: SIGPIPE y el tamaño de la terminal |

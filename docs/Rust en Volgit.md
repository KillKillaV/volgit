---
tags: [volgit, rust, aprendizaje]
---

# Rust en Volgit

Volver a [[Volgit]]

Chuleta de los conceptos de Rust que aparecen en el proyecto, cada uno con un ejemplo sacado del propio código.

## Tipos básicos que salen mucho

| Tipo | Qué es |
|---|---|
| `String` | Texto **propio**: el dueño de la memoria, se puede modificar |
| `&str` | Texto **prestado**: una vista de un texto que es de otro |
| `Vec<T>` | Lista dinámica de `T` |
| `&[T]` | Vista prestada de una lista (*slice*) |
| `u64`, `usize` | Enteros sin signo (`usize` = tamaño de memoria, índices) |
| `f32`, `f64` | Decimales |
| `(A, B)` | Tupla: por ejemplo, `(String, u64)` = nombre de lenguaje y bytes |
| `HashMap<K, V>` | Diccionario clave → valor |

## Propiedad y préstamos (`&`)

Cada valor tiene **un único dueño**. Cuando una función solo necesita **leer** algo, lo pide prestado con `&`:

```rust
pub fn print(rep: &Report, avatar: Option<&Avatar>)   // lee el informe, no se lo queda
```

Para **modificar** algo prestado hace falta `&mut`:

```rust
fn round_corners(img: &mut RgbaImage, r: f32)   // cambia píxeles de la imagen original
```

Si no se pone `&`, la función **se queda** el valor y el llamador ya no puede usarlo. Por eso `.clone()` aparece de vez en cuando: hace una copia cuando hace falta tener dos dueños.

## `Option<T>`: puede haber valor o no

En Rust **no existe `null`**. Un valor que puede faltar se declara como `Option<T>`, que es `Some(valor)` o `None`, y el compilador obliga a tratar los dos casos.

Métodos que salen mucho en el código:

```rust
opt.unwrap_or(x)            // el valor, o x si es None
opt.unwrap_or_default()     // el valor, o el "vacío" del tipo ("" , 0, vec![])
opt.map(|v| ...)            // transforma el valor si existe
opt.map_or(x, |v| ...)      // transforma, o usa x si es None
opt.filter(|v| cond)        // lo convierte en None si no cumple la condición
cond.then(|| valor)         // bool → Some(valor) si es true, None si es false
```

Ejemplo real, que combina tres: "la descripción, si existe **y** no está vacía":

```rust
if let Some(d) = r.description.as_deref().filter(|d| !d.is_empty()) { ... }
```

`as_deref()` convierte `Option<String>` en `Option<&str>`, es decir, presta el texto en lugar de moverlo.

## `Result<T>` y el operador `?`

Una operación que puede fallar devuelve `Result`, que es `Ok(valor)` o `Err(error)`. El operador **`?`** significa "si es error, devuélvelo hacia arriba y sal de la función; si no, dame el valor":

```rust
let resp = req.send().with_context(|| format!("fallo de red en {path}"))?;
```

El proyecto usa la librería **`anyhow`**:
- `anyhow::Result<T>` es un `Result` que admite cualquier tipo de error.
- `bail!("mensaje")` devuelve un error al momento.
- `.context("...")` y `.with_context(|| ...)` añaden explicación al error.

Como `main` devuelve `Result<()>`, cualquier error acaba impreso como `Error: mensaje`.

`?` también funciona con `Option` dentro de funciones que devuelven `Option`:

```rust
let (owner, name) = (parts.next()?, parts.next()?);   // si falta alguno → return None
```

## `match`: el switch con superpoderes

`match` obliga a cubrir **todos** los casos posibles, y admite condiciones extra (*guards*) con `if`:

```rust
match resp.status() {
    StatusCode::NOT_FOUND => Ok(None),
    StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => bail!("límite..."),  // | = "o"
    s if !s.is_success() => bail!("GitHub respondió {s}"),                        // guard
    _ => Ok(Some(resp.json()?)),                                                  // _ = cualquier otro
}
```

También se puede hacer `match` sobre rangos (`d if d < 30`) y sobre tuplas: ver `cell()` en [[Foto de perfil#Pintar una celda]].

## let-else

```rust
let Some((tag, detail)) = describe(e) else { continue };
```

Significa: "si `describe` devuelve `Some`, saca `tag` y `detail`; si no, ejecuta el `else`". El `else` debe salir del bloque, con `continue`, `return` o `break`. Sirve para evitar anidar `if`s.

## Closures: funciones anónimas

`|argumentos| cuerpo` es una función sin nombre. Aparecen por todas partes:

```rust
repos.retain(|r| !r.fork);                                      // quedarse con los que no son fork
repos.sort_by(|a, b| b.stargazers_count.cmp(&a.stargazers_count));   // ordenar descendente
```

## Iteradores

En vez de bucles `for` con índices, Rust encadena operaciones sobre iteradores:

```rust
let total: u64 = langs.iter().map(|l| l.1).sum();
//               recorrer   → coger los bytes → sumar
```

Los más usados son `iter`, `map`, `filter`, `filter_map` (filtra y transforma a la vez), `take(n)`, `zip` (recorre dos listas en paralelo), `collect` (junta el resultado en un `Vec` o un `String`) y `sum`.

## Structs, `impl` y `derive`

```rust
pub struct GitHub {                 // datos
    client: Client,
    token: Option<String>,
}

impl GitHub {                       // métodos
    pub fn new(token: Option<String>) -> Result<Self> { ... }   // "constructor" (convención)
    fn get<T>(&self, path: &str) ... { ... }                     // &self = método de instancia
}
```

`#[derive(...)]` genera código automáticamente:
- `Debug` permite imprimir el struct con `{:?}`.
- `Deserialize` permite construirlo desde JSON.
- `Serialize` permite convertirlo a JSON.
- `Parser` (de clap) permite construirlo desde los argumentos de la terminal.

`pub` significa "visible desde otros módulos". Sin `pub`, es privado del archivo.

## Genéricos y traits

```rust
fn get<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>>
```

`T` es "un tipo cualquiera", y `: DeserializeOwned` es una **restricción**: tiene que ser un tipo que se pueda construir desde JSON. Un *trait* es como una interfaz, un conjunto de capacidades que un tipo implementa. Ver [[API de GitHub#La función get]].

`impl std::fmt::Display` en un parámetro significa "cualquier cosa que se pueda imprimir":

```rust
fn kv(&self, label: &str, value: impl std::fmt::Display) -> String
```

Por eso a `kv` se le puede pasar un `String`, un `&str` o un texto con color.

## Módulos

Cada archivo de `src/` es un módulo. `main.rs` los declara con `mod github;` y los usa con `github::GitHub::new(...)`. Dentro de un módulo, `use crate::avatar::Avatar;` importa algo de otro módulo del mismo proyecto.

## Tests

```rust
#[cfg(test)]           // este bloque solo se compila al ejecutar `cargo test`
mod tests {
    use super::*;      // importa todo lo del archivo que lo contiene

    #[test]
    fn usuarios() {
        assert_eq!(parse_user("@BurntSushi").as_deref(), Some("BurntSushi"));
    }
}
```

Se ejecutan con `cargo test`. Hay tests en `main.rs` (lectura de la entrada) y en `render.rs` (`distribute` y `wrap`).

## unsafe y cfg

- `unsafe { ... }` desactiva algunas comprobaciones del compilador. Solo hace falta para cosas que Rust no puede verificar, como llamar a funciones de C. En el proyecto aparece una sola vez: ver [[Arquitectura#SIGPIPE y unsafe]].
- `#[cfg(unix)]` compila un trozo solo en ciertos sistemas. `#[cfg(test)]` lo compila solo en los tests.

## Dependencias (`Cargo.toml`)

| Crate | Para qué |
|---|---|
| `clap` | Argumentos de la terminal y `--help` |
| `reqwest` | Peticiones HTTP |
| `serde` / `serde_json` | JSON ↔ structs |
| `anyhow` | Errores cómodos |
| `colored` | Colores en la terminal |
| `chrono` | Fechas ("hace 3 días") |
| `image` | Decodificar y redimensionar la foto |
| `libc` | Acceso a funciones del sistema (SIGPIPE) |

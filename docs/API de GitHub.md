---
tags: [volgit, github, api]
---

# API de GitHub

Volver a [[Volgit]] · Código en `src/github.rs`

## Endpoints usados

**Ficha de repositorio**. En total son 5 peticiones:

| Endpoint | Para qué |
|---|---|
| `GET /repos/{o}/{r}` | Datos generales: estrellas, forks, licencia, fechas… |
| `GET /repos/{o}/{r}/languages` | Bytes de código por lenguaje |
| `GET /repos/{o}/{r}/contributors?per_page=N` | Top contribuidores |
| `GET /repos/{o}/{r}/releases/latest` | Última release (404 si no hay) |
| `GET /search/issues?q=repo:{o}/{r}+type:pr+state:open` | Número de PRs abiertos |

**Perfil**. En total son de 3 a 7 peticiones:

| Endpoint | Para qué |
|---|---|
| `GET /users/{u}` | Datos del perfil (vale igual para organizaciones) |
| `GET /users/{u}/repos?per_page=100&page=N` | Sus repos, de 1 a 5 páginas |
| `GET /users/{u}/events/public` | Actividad reciente (últimos 90 días, máx. 30 eventos) |

Con `--panel` se añade una consulta a la **API GraphQL** (`POST /graphql`, requiere token), que devuelve el calendario de contribuciones: 53 semanas con el nivel de cada día (`NONE` … `FOURTH_QUARTILE`) y los totales de commits, PRs, issues y revisiones. Es la única forma de obtenerlo, porque la API REST no lo ofrece.

La foto se descarga aparte con la URL `avatar_url` que viene en la respuesta. No cuenta para el límite de la API porque se sirve desde otro dominio.

## De JSON a structs: serde

Cada respuesta se convierte automáticamente en un struct de Rust:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub login: String,
    #[serde(rename = "type")]   // "type" es palabra reservada en Rust:
    pub kind: String,           // el campo JSON "type" se guarda en `kind`
    pub name: Option<String>,   // Option: el campo puede venir como null
    pub followers: u64,
    // ...
}
```

- **Solo se declaran los campos que usamos.** GitHub envía decenas de campos más y serde ignora el resto.
- Si un campo puede ser `null` en el JSON y lo declaras sin `Option`, la deserialización falla. Por eso `name`, `bio`, `company`… son `Option<String>`.
- `#[serde(default)]` en `topics` significa que, si el campo no aparece, se usa un `Vec` vacío en vez de fallar.
- `Serialize` es lo que permite a `--json` volcar los mismos structs.

### Eventos: JSON sin struct

Cada tipo de evento (push, PR, issue…) tiene un `payload` con forma distinta. En lugar de declarar un struct por tipo, el payload se guarda como JSON genérico:

```rust
pub struct Event {
    #[serde(rename = "type")]
    pub kind: String,
    pub repo: EventRepo,
    pub created_at: String,
    #[serde(default)]
    pub payload: serde_json::Value,   // JSON sin tipar
}
```

Después se lee con índices: `p["pull_request"]["title"].as_str()`. Si una clave no existe, `serde_json` devuelve `Null` en vez de fallar, y `.as_str()` da `None`. Es más cómodo para datos variables, a cambio de perder las comprobaciones del compilador.

## La función get

Todas las consultas pasan por aquí:

```rust
/// GET a la API. Devuelve Ok(None) en 404 (p.ej. repo sin releases).
fn get<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
    let mut req = self
        .client
        .get(format!("{API}{path}"))
        .header("Accept", "application/vnd.github+json");
    if let Some(t) = &self.token {
        req = req.bearer_auth(t);            // "Authorization: Bearer <token>"
    }
    let resp = req.send().with_context(|| format!("fallo de red en {path}"))?;
    match resp.status() {
        StatusCode::NOT_FOUND => Ok(None),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => bail!(
            "límite de peticiones de GitHub alcanzado; exporta GITHUB_TOKEN para subirlo"
        ),
        s if !s.is_success() => bail!("GitHub respondió {s} en {path}"),
        _ => Ok(Some(resp.json()?)),         // JSON → T
    }
}
```

Tres cosas a destacar:

1. **Genérica:** `get<T: DeserializeOwned>` significa "dame cualquier tipo que se pueda construir desde JSON". El tipo concreto lo deduce el compilador de lo que esperas recibir:
   ```rust
   let repo: Repo = self.get(&base)?...;                      // T = Repo
   let langs: HashMap<String, u64> = self.get(...)?...;       // T = HashMap
   ```
2. **El tipo de retorno `Result<Option<T>>` distingue tres casos:**
   - `Ok(Some(dato))` → todo bien.
   - `Ok(None)` → 404, "no existe". No es un error para nosotros, porque un repo sin releases es normal.
   - `Err(...)` → fallo de red, límite agotado, error 500…
3. `with_context` añade una explicación al error. Si falla la red, verás "fallo de red en /repos/x/y" en lugar de un error críptico de la librería.

Cada llamador decide qué hacer con el `None`:

```rust
// Obligatorio: si no existe el repo, es un error con mensaje claro.
let repo: Repo = self.get(&base)?
    .with_context(|| format!("no existe el repo {owner}/{name} (o es privado)"))?;

// Opcional: si no hay lenguajes, usamos un mapa vacío.
let langs: HashMap<String, u64> = self.get(...)?.unwrap_or_default();
```

## Separar issues de PRs

El campo `open_issues_count` de GitHub **cuenta issues y PRs juntos**. Para separarlos se usa la búsqueda:

```rust
// La búsqueda tiene un límite más estricto; si falla no rompemos el informe.
let open_prs = self
    .get::<SearchCount>(&format!(
        "/search/issues?q=repo:{owner}/{name}+type:pr+state:open&per_page=1"
    ))
    .ok()        // Result → Option: si hay error, None (ignoramos el error)
    .flatten()   // Option<Option<T>> → Option<T>
    .map(|s| s.total_count);
```

Si la búsqueda falla, la ficha no se rompe: muestra "Issues X abiertos (con PRs)". El `get::<SearchCount>` con `::<>` (llamado *turbofish*) indica el tipo `T` explícitamente, porque aquí el compilador no lo puede deducir.

## Paginación de repos

```rust
// Máximo 5 páginas (500 repos) para no fundir el límite de la API.
let pages = user.public_repos.div_ceil(100).clamp(1, 5);
for page in 1..=pages {
    let batch: Vec<UserRepo> = self
        .get(&format!("/users/{login}/repos?per_page=100&type=owner&page={page}"))?
        .unwrap_or_default();
    repos.extend(batch);
}
repos.retain(|r| !r.fork);                                   // fuera los forks
repos.sort_by(|a, b| b.stargazers_count.cmp(&a.stargazers_count)); // b vs a = descendente
```

- `div_ceil(100)` es la división redondeando hacia arriba: 182 repos dan 2 páginas.
- `clamp(1, 5)` acota el resultado entre 1 y 5.
- **Limitación:** en cuentas con más de 500 repos, las estrellas totales y los lenguajes solo cuentan los primeros 500.

## Límites

| | Peticiones por hora |
|---|---|
| Sin token | 60 por IP |
| Con token | 5000 |
| Búsqueda | 10 por minuto sin token / 30 con token |

Si se agota el límite, GitHub responde 403 o 429 y Volgit muestra el aviso del token. Ver [[Uso#El token de GitHub]].

**Mejoras pendientes:**
- Mostrar a qué hora se reinicia el límite (cabecera `X-RateLimit-Reset`).
- Caché local.
- Usar GraphQL para bajar de 7 peticiones a 1 o 2.

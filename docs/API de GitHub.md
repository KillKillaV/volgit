---
tags: [volgit, github, api]
---

# API de GitHub

Volver a [[Volgit]] · El código está en `src/github.rs`

## Qué le pido a GitHub

Para la ficha de un repo hago 5 peticiones:

| Endpoint | Para qué |
|---|---|
| `GET /repos/{o}/{r}` | Lo general: estrellas, forks, licencia, fechas... |
| `GET /repos/{o}/{r}/languages` | Bytes de código de cada lenguaje |
| `GET /repos/{o}/{r}/contributors?per_page=N` | Los que más contribuyen |
| `GET /repos/{o}/{r}/releases/latest` | La última release (da 404 si no hay) |
| `GET /search/issues?q=repo:{o}/{r}+type:pr+state:open` | Cuántos PRs abiertos hay |

Para un perfil, entre 3 y 7:

| Endpoint | Para qué |
|---|---|
| `GET /users/{u}` | Los datos del perfil (sirve igual para organizaciones) |
| `GET /users/{u}/repos?per_page=100&page=N` | Sus repos, de 1 a 5 páginas |
| `GET /users/{u}/events/public` | La actividad reciente (máximo 30 eventos de los últimos 90 días) |

Y si pongo `--panel`, una más a la API GraphQL (`POST /graphql`) para el calendario de contribuciones. Es la única manera de sacarlo, porque la API normal (la REST) no lo da, y GraphQL exige token siempre. Me devuelve las 53 semanas con el nivel de cada día (de `NONE` a `FOURTH_QUARTILE`) y los totales de commits, PRs, issues y revisiones. Solo la pido si uso `--panel`, para no gastar una petición de más.

La foto va aparte, con la `avatar_url` que viene en la respuesta. Esa no cuenta para el límite porque sale de otro dominio.

## De JSON a structs con serde

Cada respuesta se convierte sola en un struct de Rust:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct User {
    pub login: String,
    #[serde(rename = "type")]   // "type" es palabra reservada en Rust,
    pub kind: String,           // así que el campo "type" del JSON lo guardo en `kind`
    pub name: Option<String>,   // Option porque puede venir null
    pub followers: u64,
    // ...
}
```

Lo que tuve que entender:
- Solo declaro los campos que uso. GitHub manda decenas más y serde pasa de ellos.
- Si un campo puede venir a `null` y no lo pongo como `Option`, la conversión falla entera. Por eso `name`, `bio`, `company` y compañía son `Option<String>`.
- `#[serde(default)]` en `topics` significa que, si el campo no viene, uso una lista vacía en vez de fallar.
- `Serialize` es lo que hace que `--json` funcione con estos mismos structs.
- En el calendario uso `#[serde(rename_all = "camelCase")]`, porque GraphQL devuelve `contributionCount` y en Rust lo quiero llamar `contribution_count`.

### Los eventos van sin struct

Cada tipo de evento (push, PR, issue...) trae un `payload` con una forma distinta. Hacer un struct para cada uno era un rollo, así que el payload lo guardo como JSON sin tipar:

```rust
pub struct Event {
    #[serde(rename = "type")]
    pub kind: String,
    pub repo: EventRepo,
    pub created_at: String,
    #[serde(default)]
    pub payload: serde_json::Value,   // JSON a pelo
}
```

Luego lo leo con índices: `p["pull_request"]["title"].as_str()`. Si la clave no existe no peta: devuelve `Null`, y `.as_str()` da `None`. Es más cómodo, a cambio de que el compilador ya no me avisa si me equivoco en un nombre.

Una cosa que descubrí probando: a veces GitHub no manda el título del PR en el evento. En ese caso enseño el número (`abre #28911`) en vez de dejar el hueco vacío.

## La función get

Todas las consultas pasan por aquí:

```rust
fn get<T: DeserializeOwned>(&self, path: &str) -> Result<Option<T>> {
    // ... monta la petición con el token si lo hay ...
    let resp = req.send().with_context(|| format!("fallo de red en {path}"))?;
    let status = resp.status();
    if status == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if status.is_success() {
        return Ok(Some(resp.json()?));
    }

    // Un 403 no siempre es el límite: GitHub también lo usa p.ej. cuando la
    // lista de contribuidores es demasiado grande. Solo es el límite si la
    // cabecera dice que no quedan peticiones (o si es un 429).
    let header = |name: &str| -> Option<i64> { resp.headers().get(name)?.to_str().ok()?.parse().ok() };
    let remaining = header("x-ratelimit-remaining");
    let reset = header("x-ratelimit-reset");
    if status == StatusCode::TOO_MANY_REQUESTS || remaining == Some(0) {
        // ... "límite agotado (se reinicia a las HH:MM)" ...
    }
    // Cualquier otro error: enseño el mensaje que da GitHub.
}
```

Lo que tiene de interesante:

1. Es genérica. `get<T: DeserializeOwned>` quiere decir "dame cualquier tipo que se pueda sacar de un JSON", y el compilador deduce cuál por lo que espero recibir:
   ```rust
   let repo: Repo = self.get(&base)?...;                      // T = Repo
   let langs: HashMap<String, u64> = self.get(...)?...;       // T = HashMap
   ```
2. Devuelve `Result<Option<T>>`, que distingue tres casos:
   - `Ok(Some(dato))`: todo bien.
   - `Ok(None)`: 404, no existe. Para mí no es un error, porque que un repo no tenga releases es lo normal.
   - `Err(...)`: fallo de red, límite agotado, error del servidor...
3. `with_context` le añade una explicación al error, así veo "fallo de red en /repos/x/y" en vez de un mensaje críptico de la librería.

### La historia del 403 con torvalds/linux

Al principio trataba cualquier 403 como "límite agotado". Al probar con `torvalds/linux` me decía que se había acabado el límite cuando tenía 5000 peticiones disponibles. Resulta que Linux tiene tantos contribuidores que GitHub se niega a listarlos y contesta con un 403. Ahora solo digo "límite agotado" si la cabecera `x-ratelimit-remaining` vale 0, y en cualquier otro caso enseño el mensaje real de GitHub. Además los contribuidores pasaron a ser opcionales: si fallan, la ficha sale igual pero sin esa sección.

### Cada llamada decide qué hacer con el None

```rust
// Obligatorio: si el repo no existe, error con un mensaje claro.
let repo: Repo = self.get(&base)?
    .with_context(|| format!("no existe el repo {owner}/{name} (o es privado)"))?;

// Opcional: si no hay lenguajes, un mapa vacío y listo.
let langs: HashMap<String, u64> = self.get(...)?.unwrap_or_default();
```

## Separar issues de PRs

El campo `open_issues_count` de GitHub mete issues y PRs en el mismo saco. Para separarlos uso el buscador:

```rust
// La búsqueda tiene un límite más estricto; si falla no rompemos el informe.
let open_prs = self
    .get::<SearchCount>(&format!(
        "/search/issues?q=repo:{owner}/{name}+type:pr+state:open&per_page=1"
    ))
    .ok()        // Result → Option: si hay error, None y me olvido del error
    .flatten()   // Option<Option<T>> → Option<T>
    .map(|s| s.total_count);
```

Si la búsqueda falla, la ficha no se rompe: pone "Issues X abiertos (con PRs)". Lo de `get::<SearchCount>` se llama *turbofish* y sirve para decirle el tipo a mano cuando el compilador no lo puede deducir.

## Los repos vienen en páginas

```rust
// Máximo 5 páginas (500 repos) para no fundir el límite de la API.
let pages = user.public_repos.div_ceil(100).clamp(1, 5);
for page in 1..=pages {
    let batch: Vec<UserRepo> = self
        .get(&format!("/users/{login}/repos?per_page=100&type=owner&page={page}"))?
        .unwrap_or_default();
    repos.extend(batch);
}
repos.retain(|r| !r.fork);                                          // fuera los forks
repos.sort_by(|a, b| b.stargazers_count.cmp(&a.stargazers_count)); // b contra a = de mayor a menor
```

- `div_ceil(100)` divide redondeando hacia arriba: 182 repos son 2 páginas.
- `clamp(1, 5)` deja el número entre 1 y 5.
- La pega: en cuentas con más de 500 repos, las estrellas totales y los lenguajes solo cuentan los 500 primeros.

## Límites

| | Peticiones |
|---|---|
| Sin token | 60 por hora y por IP |
| Con token | 5000 por hora |
| Buscador | 10 por minuto sin token, 30 con token |

Cosas que me gustaría hacer algún día:
- Una caché local para que repetir una consulta no gaste peticiones.
- Pasar todo a GraphQL para bajar de 7 peticiones a 1 o 2.

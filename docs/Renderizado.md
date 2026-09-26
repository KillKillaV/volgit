---
tags: [volgit, render, terminal]
---

# Renderizado

Volver a [[Volgit]] · El código está en `src/render.rs`

## Cómo está montada la salida

```
  ┌──────────┐   titulo  subtitulo
  │          │   ─────────────────
  │   foto   │   descripción
  │          │
  │          │   Etiqueta    valor
  └──────────┘   Etiqueta    valor
                 ...
  Sección                         ← debajo, a lo ancho
  ━━━━━━━━━━━━━━━━━━━━━━━
```

Lo hago en dos pasos:
1. Meto las líneas de la derecha en un `Vec<String>` (`info`).
2. `side_by_side` las junta con las líneas de la foto.

Las secciones de abajo (panel, lenguajes, repos, actividad) las imprimo directamente.

## El lío de medir texto con color

Esta fue la trampa más gorda del render. Un texto con color como

```
"\x1b[1;38;2;122;162;247mEstrellas\x1b[0m"
```

se ve de 9 caracteres pero mide 31. Si lo alineo con `format!("{:<12}", texto)`, Rust cuenta 31, no añade ningún espacio y se descuadra todo.

Así que tengo una regla: primero relleno el texto plano y después le pongo color.

```rust
fn kv(&self, label: &str, value: impl std::fmt::Display) -> String {
    //                   ↓ relleno el texto plano a 12...         ↓ ...y luego lo coloreo
    format!("{} {value}", self.a(&format!("{label:<LABEL_W$}")).bold())
}
```

En la leyenda de lenguajes hago lo mismo pero calculando el relleno a mano:

```rust
let pct = format!("{:.1}%", *n as f64 * 100.0 / total as f64);
// El padding se calcula sobre el texto plano para no contar los códigos ANSI.
let pad = 22usize.saturating_sub(name.chars().count() + pct.len() + 3);
format!("{} {name} {}{}", "●".truecolor(r, g, b), pct.dimmed(), " ".repeat(pad))
```

- `saturating_sub` es una resta que se queda en 0 en vez de pasarse. Con `usize`, que no tiene signo, `3 - 5` haría *panic*.
- Uso `.chars().count()` y no `.len()` porque `len()` cuenta bytes. "Ubicación" tiene 9 letras pero 10 bytes, porque la `ó` ocupa 2. Esto me mordió de verdad con el "Más" de la leyenda del panel.

## Foto y texto en paralelo

```rust
/// Imprime el avatar a la izquierda y `info` a la derecha, fila a fila.
fn side_by_side(avatar: Option<&Avatar>, info: &[String]) {
    println!();
    // Sin foto, la columna de la izquierda es una lista vacía.
    let left = avatar.map_or(&[][..], |a| &a.lines[..]);
    // Relleno para las filas en las que la foto ya se acabó pero el texto sigue.
    let blank = avatar.map_or(String::new(), |a| format!("{}{GAP}", " ".repeat(a.width)));
    for i in 0..left.len().max(info.len()) {
        let l = left.get(i).map_or(blank.clone(), |l| format!("{l}{GAP}"));
        let r = info.get(i).map_or("", String::as_str);
        println!("{MARGIN}{l}{r}");
    }
}
```

Aquí no tengo que medir nada porque ya sé lo que ocupa la foto (`a.width`). Recorro hasta la columna más larga, y `get(i)` me da `None` cuando una se acaba.

- `map_or(por_defecto, función)` es "si hay algo, aplícale la función, y si no, usa lo de por defecto".
- `&[][..]` es un *slice* vacío, o sea, "ninguna línea".

Esta función funciona igual con los bloques que con kitty. Con kitty, la primera línea de la foto lleva la imagen escondida dentro, y el resto son espacios que guardan el hueco. Ver [[Foto de perfil#Encajarla con el texto]].

## La barra de lenguajes

La barra mide 48 columnas y a cada lenguaje le toca un trozo proporcional. Parece fácil, pero tiene dos pegas:

1. El redondeo no cuadra. Con porcentajes como 95.5, 4.1 y 0.4, la suma de los redondeos puede dar 47 o 49.
2. Todos se tienen que ver. Un 0.4% serían 0.19 columnas, así que le garantizo al menos 1.

Lo resolví con el método del mayor resto, que es el mismo que se usa para repartir escaños en unas elecciones:

```rust
fn distribute(values: &[u64], total: usize) -> Vec<usize> {
    let sum: u64 = values.iter().sum();
    // Anchos exactos con decimales: [45.84, 1.97, 0.19]
    let exact: Vec<f64> = values.iter().map(|&v| v as f64 * total as f64 / sum as f64).collect();
    // Parte entera, con mínimo 1: [45, 1, 1], que suman 47
    let mut out: Vec<usize> = exact.iter().map(|e| (e.floor() as usize).max(1)).collect();
    let mut used: usize = out.iter().sum();

    // Faltan columnas por el redondeo: se las doy a los de mayor parte decimal.
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())));
    for &i in &order {
        if used >= total {
            break;
        }
        out[i] += 1;                        // 1.97 tiene el resto más grande (.97): [45, 2, 1] = 48
        used += 1;
    }
    // Sobran columnas por culpa del mínimo de 1: se las quito al más ancho.
    while used > total {
        let Some(i) = (0..out.len()).filter(|&i| out[i] > 1).max_by_key(|&i| out[i]) else { break };
        out[i] -= 1;
        used -= 1;
    }
    out
}
```

`total_cmp` es para comparar `f64`, que no se pueden ordenar con el `cmp` normal porque existe `NaN`. El último bucle lo añadí después: con varios lenguajes muy pequeños, el mínimo de 1 hacía que la barra se pasara de 48. El test `distribute_suma_exacta` comprueba que siempre dé 48 justas.

## El panel de contribuciones (--panel)

Son los cuadraditos verdes del perfil de GitHub: una columna por semana y una fila por día, empezando en domingo como hace GitHub. Los datos vienen de GraphQL (ver [[API de GitHub]]) y cada día trae su nivel, de `NONE` a `FOURTH_QUARTILE`, que convierto en uno de estos cinco verdes:

```rust
/// Colores de GitHub en modo oscuro, de "sin contribuciones" a "máximo".
const CAL_LEVELS: [Rgb; 5] = [(45, 51, 59), (14, 68, 41), (0, 109, 50), (38, 166, 65), (57, 211, 83)];
```

Cada semana ocupa 2 columnas (`"■ "`), así que el año entero son unas 110 columnas. Si la terminal es más estrecha, enseño solo las semanas más recientes que caben:

```rust
let fit = crate::term::width().saturating_sub(MARGIN.len() + CAL_LABEL_W) / 2;
let weeks = &cal.weeks[cal.weeks.len().saturating_sub(fit)..];
```

El ancho lo saco con `term::width()`, que se lo pregunta al sistema con `ioctl`.

Lo que más guerra dio fueron los meses de arriba. Cada etiqueta va en la semana en la que cambia el mes, pero si el primer mes solo asoma una o dos semanas, su etiqueta se pegaba a la siguiente ("sep oct"). GitHub en ese caso no pone la primera, así que hice lo mismo:

```rust
// Como GitHub: si el primer mes solo asoma una o dos semanas, no se etiqueta.
if starts.len() >= 2 && starts[1].0 - starts[0].0 < 3 {
    starts.remove(0);
}
```

Debajo va el reparto por tipo (commits, PRs, revisiones, issues) a la izquierda y la leyenda "Menos ■■■■■ Más" a la derecha. Si no caben en una línea, van una debajo de la otra.

## Partir texto en líneas

`wrap(texto, ancho, max_lineas)` parte las descripciones sin cortar palabras. Si sobran líneas, la última termina en `…`. Tiene su test, `wrap_no_pasa_del_ancho`.

## La actividad

`describe` convierte cada evento de GitHub en una etiqueta corta y un detalle. Los que no me interesan devuelven `None` y desaparecen:

```rust
"PullRequestEvent" => {
    // GitHub no tiene acción "merged": un PR mergeado es "closed" con merged = true.
    let merged = action == "closed" && p["pull_request"]["merged"].as_bool() == Some(true);
    let tag = if merged { "merge" } else { "pr" };
    (tag, with_verb(verb(action, merged), &title(&p["pull_request"], p)))
}
```

`title` saca el título, y si GitHub no lo manda usa el número (`#123`). `with_verb` lo junta con el verbo: "abre: Arregla X" o "abre #123".

Después `activity` junta los eventos seguidos que son iguales, para que 5 pushes a la misma rama no ocupen 5 líneas:

```rust
let mut rows: Vec<(&Event, &str, String, usize)> = vec![];   // (evento, etiqueta, detalle, veces)
for e in events {
    let Some((tag, detail)) = describe(e) else { continue };  // los que no interesan, fuera
    // ¿Es igual que la última fila? Entonces solo sumo 1 al contador.
    if let Some(last) = rows.last_mut().filter(|l| l.1 == tag && l.0.repo.name == e.repo.name && l.2 == detail) {
        last.3 += 1;
        continue;
    }
    rows.push((e, tag, detail, 1));
}
```

- `last_mut()` me da la última fila de forma que la puedo modificar, así cambio su contador sin sacarla del vector.
- `let Some(x) = ... else { continue };` es un *let-else*: si no encaja, se ejecuta el `else`, que tiene que salir del bloque. Ver [[Rust en Volgit#let-else]].

## El color de acento

El struct `Theme` guarda el color que saqué de la foto (ver [[Foto de perfil#Color de acento]]). Todas las etiquetas y títulos se pintan con `t.a("texto")`, así que si algún día quiero cambiar cómo se colorea, lo toco en un solo sitio.

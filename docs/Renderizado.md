---
tags: [volgit, render, terminal]
---

# Renderizado

Volver a [[Volgit]] · Código en `src/render.rs`

## Estructura de la salida

```
  ┌──────────┐   titulo  subtitulo
  │          │   ─────────────────
  │   foto   │   descripción
  │          │
  │          │   Etiqueta    valor
  └──────────┘   Etiqueta    valor
                 ...
  Sección                         ← a lo ancho, debajo
  ━━━━━━━━━━━━━━━━━━━━━━━
```

El render se hace en dos fases:
1. Se construye un `Vec<String>` con las líneas de la derecha (`info`).
2. `side_by_side` las junta con las líneas de la foto.

Las secciones de abajo (lenguajes, repos, actividad) se imprimen directamente.

## El problema de medir texto con color

Esta es la trampa más importante de todo el render. Un texto con color como

```
"\x1b[1;38;2;122;162;247mEstrellas\x1b[0m"
```

**se ve** de 9 caracteres pero **mide** 31. Si se alinea con `format!("{:<12}", texto)`, Rust cuenta 31, no añade relleno y todo queda descuadrado.

**Regla del proyecto: primero se rellena el texto plano y después se colorea.**

```rust
fn kv(&self, label: &str, value: impl std::fmt::Display) -> String {
    //                   ↓ se rellena el texto plano a 12...        ↓ ...y luego se colorea
    format!("{} {value}", self.a(&format!("{label:<LABEL_W$}")).bold())
}
```

En la leyenda de lenguajes se aplica la misma idea, calculando el relleno a mano:

```rust
let pct = format!("{:.1}%", *n as f64 * 100.0 / total as f64);
// El padding se calcula sobre el texto plano para no contar los códigos ANSI.
let pad = 22usize.saturating_sub(name.chars().count() + pct.len() + 3);
format!("{} {name} {}{}", "●".truecolor(r, g, b), pct.dimmed(), " ".repeat(pad))
```

- `saturating_sub` es una resta que se queda en 0 en lugar de desbordar. Con `usize`, sin signo, `3 - 5` haría un *panic*.
- Se usa `.chars().count()` y no `.len()`, porque `len()` cuenta **bytes**: "Ubicación" tiene 9 letras pero 10 bytes (la `ó` ocupa 2).

## Foto y texto en paralelo

```rust
/// Imprime el avatar a la izquierda y `info` a la derecha, fila a fila.
fn side_by_side(avatar: Option<&Avatar>, info: &[String]) {
    println!();
    // Sin foto, la columna izquierda es una lista vacía.
    let left = avatar.map_or(&[][..], |a| &a.lines[..]);
    // Relleno para las filas donde la foto ya se acabó pero el texto sigue.
    let blank = avatar.map_or(String::new(), |a| format!("{}{GAP}", " ".repeat(a.width)));
    for i in 0..left.len().max(info.len()) {
        let l = left.get(i).map_or(blank.clone(), |l| format!("{l}{GAP}"));
        let r = info.get(i).map_or("", String::as_str);
        println!("{MARGIN}{l}{r}");
    }
}
```

Aquí no hace falta medir nada, porque el ancho de la foto se conoce de antemano (`a.width`). Se itera hasta la columna más larga, y `get(i)` devuelve `None` cuando una se acaba.

- `map_or(por_defecto, función)` significa "si hay valor, aplícale la función; si no, usa el valor por defecto".
- `&[][..]` es un *slice* vacío, que representa "ninguna línea".

## Barra de lenguajes: repartir columnas

La barra mide 48 columnas. Hay que dar a cada lenguaje un ancho proporcional, y tiene dos dificultades:

1. **El redondeo no cuadra.** Con 3 lenguajes al 33.3%, cada uno recibe 16 columnas y la suma da 48, pero con porcentajes como 95.5 / 4.1 / 0.4 la suma de los redondeos puede dar 47 o 49.
2. **Todos deben verse.** Un 0.4% daría 0.19 columnas, así que se le garantiza 1.

La solución es el **método del mayor resto**, el mismo que se usa para repartir escaños:

```rust
fn distribute(values: &[u64], total: usize) -> Vec<usize> {
    let sum: u64 = values.iter().sum();
    // Anchos exactos con decimales: [45.84, 1.97, 0.19]
    let exact: Vec<f64> = values.iter().map(|&v| v as f64 * total as f64 / sum as f64).collect();
    // Parte entera, con mínimo 1:            [45, 1, 1]  → suman 47
    let mut out: Vec<usize> = exact.iter().map(|e| (e.floor() as usize).max(1)).collect();
    let mut used: usize = out.iter().sum();

    // Faltan columnas por el redondeo: se las damos a los de mayor parte decimal.
    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|&a, &b| (exact[b] - exact[b].floor()).total_cmp(&(exact[a] - exact[a].floor())));
    for &i in &order {
        if used >= total {
            break;
        }
        out[i] += 1;                        // 1.97 tiene el mayor resto (.97) → [45, 2, 1] = 48
        used += 1;
    }
    // Sobran columnas por el mínimo de 1: se las quitamos al más ancho.
    while used > total {
        let Some(i) = (0..out.len()).filter(|&i| out[i] > 1).max_by_key(|&i| out[i]) else { break };
        out[i] -= 1;
        used -= 1;
    }
    out
}
```

`total_cmp` se usa para comparar `f64`: los decimales no se pueden ordenar con `cmp` normal porque existe `NaN`. Esta función tiene test: `distribute_suma_exacta` comprueba que siempre suma exactamente 48.

## Partir texto en líneas

`wrap(texto, ancho, max_lineas)` parte las descripciones sin cortar palabras. Si sobran líneas, la última termina en `…`. Tiene su test `wrap_no_pasa_del_ancho`.

## Actividad: describir y agrupar eventos

`describe` traduce cada evento de GitHub a una etiqueta corta y un detalle. Los eventos que no interesan devuelven `None` y se descartan:

```rust
"PullRequestEvent" => {
    // GitHub no tiene acción "merged": un PR mergeado es "closed" con merged = true.
    let merged = action == "closed" && p["pull_request"]["merged"].as_bool() == Some(true);
    let tag = if merged { "merge" } else { "pr" };
    // ...
}
```

Después, `activity` **agrupa eventos seguidos idénticos**. Así 5 pushes a la misma rama no ocupan 5 líneas:

```rust
let mut rows: Vec<(&Event, &str, String, usize)> = vec![];   // (evento, etiqueta, detalle, veces)
for e in events {
    let Some((tag, detail)) = describe(e) else { continue };  // eventos ignorados: saltar
    // ¿Es igual que la última fila? Entonces solo sumamos 1 al contador.
    if let Some(last) = rows.last_mut().filter(|l| l.1 == tag && l.0.repo.name == e.repo.name && l.2 == detail) {
        last.3 += 1;
        continue;
    }
    rows.push((e, tag, detail, 1));
}
```

- `last_mut()` da una referencia **mutable** a la última fila, lo que permite modificar su contador sin sacarla del vector.
- `let Some(x) = ... else { continue };` es un *let-else*: si el patrón no encaja, se ejecuta el `else`, que debe salir del bloque. Ver [[Rust en Volgit#let-else]].

## Color de acento

El struct `Theme` guarda el color sacado de la foto (ver [[Foto de perfil#Color de acento]]). Todas las etiquetas y títulos se pintan con `t.a("texto")`. Así el color se cambia en un solo sitio.

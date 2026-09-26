---
tags: [volgit, imagen, ansi]
---

# Foto de perfil

Volver a [[Volgit]] · Código en `src/avatar.rs`

Es la parte con más truco del proyecto: dibujar una foto usando solo caracteres de texto.

## La idea: medios bloques

Una celda de la terminal es un rectángulo el doble de alto que de ancho. Si la pintas entera, cada "píxel" sale alargado. El truco es el carácter **`▀` (medio bloque superior)**:

```
┌───┐
│▀▀▀│ ← color del TEXTO  = píxel de arriba
│   │ ← color del FONDO  = píxel de abajo
└───┘
```

Así, **cada celda pinta 2 píxeles cuadrados**. Una foto de 28×28 píxeles ocupa 28 columnas × 14 filas.

## Los colores: secuencias ANSI

Para cambiar colores la terminal entiende unas secuencias especiales que empiezan por el carácter ESC (`\x1b`):

| Secuencia | Efecto |
|---|---|
| `\x1b[38;2;R;G;Bm` | Color de texto RGB (38 = texto, 2 = modo 24 bits) |
| `\x1b[48;2;R;G;Bm` | Color de fondo RGB (48 = fondo) |
| `\x1b[49m` | Fondo por defecto de la terminal (transparente) |
| `\x1b[0m` | Quitar todos los estilos |

Por ejemplo, `\x1b[38;2;255;0;0m\x1b[48;2;0;0;255m▀` pinta una celda roja arriba y azul abajo.

## Pintar una celda

```rust
fn cell(top: &Rgba<u8>, bottom: Option<&Rgba<u8>>) -> String {
    let bottom = bottom.filter(|p| visible(p));
    match (visible(top), bottom) {
        // Los dos píxeles visibles: ▀ con texto = arriba y fondo = abajo.
        (true, Some(b)) => format!(
            "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
            top[0], top[1], top[2], b[0], b[1], b[2]
        ),
        // Solo el de arriba: ▀ sobre el fondo de la terminal.
        (true, None) => format!("\x1b[49m\x1b[38;2;{};{};{}m▀", top[0], top[1], top[2]),
        // Solo el de abajo: usamos ▄ (medio bloque INFERIOR) con el color de texto.
        (false, Some(b)) => format!("\x1b[49m\x1b[38;2;{};{};{}m▄", b[0], b[1], b[2]),
        // Ninguno: un espacio normal.
        (false, None) => "\x1b[0m ".to_string(),
    }
}
```

**¿Por qué 4 casos?** Por las esquinas redondeadas, que son píxeles transparentes. Si en una esquina se pintara el píxel transparente como negro, en una terminal con fondo gris se vería un cuadrado negro. Por eso los transparentes nunca usan color: dejan ver el fondo de la terminal, sea cual sea.

- `bottom` es un `Option` porque, si la imagen tiene un número impar de filas, la última celda no tiene píxel de abajo.
- Hacer `match` sobre la tupla `(visible(top), bottom)` permite cubrir las 4 combinaciones de golpe. El compilador obliga a cubrirlas todas.

## De imagen a líneas

```rust
pub fn from_bytes(bytes: &[u8], cols: usize) -> Option<Self> {
    // Detecta si es PNG/JPEG/GIF por los primeros bytes y la decodifica.
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = cols as u32;
    // Lanczos3 conserva mejor los detalles al reducir tanto la imagen.
    let mut img = image::imageops::resize(&img, size, size, FilterType::Lanczos3);
    round_corners(&mut img, size as f32 * 0.22);

    let mut lines = Vec::with_capacity(cols / 2 + 1);
    for y in (0..size).step_by(2) {            // de 2 en 2 filas: cada celda son 2 píxeles
        let mut line = String::new();
        for x in 0..size {
            let top = img.get_pixel(x, y);
            let bottom = (y + 1 < size).then(|| img.get_pixel(x, y + 1));
            line += &cell(top, bottom);
        }
        line += "\x1b[0m";                     // resetear al final para no "manchar" el texto de al lado
        lines.push(line);
    }
    Some(Self { lines, width: cols, accent: accent(&img) })
}
```

- **Filtro de reducción:** al pasar de 112 píxeles a 28, un filtro simple (vecino más cercano) coge 1 de cada 4 píxeles y la cara sale ruidosa. Lanczos3 promedia los vecinos con pesos y da el resultado más nítido posible a esa resolución.
- **Tamaño pedido a GitHub:** en `main.rs` la foto se pide ya a `cols × 4` píxeles (parámetro `s=` de la URL). Así se descarga poco y el filtro aún tiene detalle con el que trabajar.
- El resultado se devuelve como `Option`: si la imagen está corrupta, no hay foto, pero el programa sigue.

## Esquinas redondeadas

```rust
/// Hace transparentes las esquinas para dar un cuadrado redondeado.
fn round_corners(img: &mut RgbaImage, r: f32) {
    let (w, h) = (img.width() as f32, img.height() as f32);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);   // centro del píxel
        let cx = px.clamp(r, w - r);
        let cy = py.clamp(r, h - r);
        if (px - cx).powi(2) + (py - cy).powi(2) > r * r {
            p[3] = 0;                                       // canal alfa = transparente
        }
    }
}
```

Este código tiene más miga de lo que parece. Imagina un rectángulo interior separado `r` píxeles de cada borde:

```
┌──────────────┐
│ ·          · │   · = zona de esquina
│   ┌──────┐   │
│   │      │   │   rectángulo interior
│   └──────┘   │
│ ·          · │
└──────────────┘
```

- `clamp` busca el **punto del rectángulo interior más cercano** al píxel.
  - Si el píxel está en un lateral, ese punto está alineado con él y la distancia es menor que `r`, así que se conserva.
  - Si está en una esquina, el punto más cercano es el vértice del rectángulo interior.
- Después, la **distancia al cuadrado** se compara con `r²`. Si es mayor, el píxel queda fuera del arco y se hace transparente.
- Se compara al cuadrado para ahorrarse la raíz cuadrada.

Con esto, las 4 esquinas salen con una sola fórmula y sin `if`s por esquina. Un círculo completo quedaba peor a 28 píxeles (muy dentado), por eso se usa un cuadrado con radio del 22% del lado.

## Color de acento

El color de las etiquetas y títulos se saca de la foto. Así cada perfil tiene su propio color.

```rust
fn accent(img: &RgbaImage) -> Rgb {
    let (mut sum, mut weight) = ([0f32; 3], 0f32);
    for p in img.pixels().filter(|p| visible(p)) {
        let (_, s, l) = to_hsl(p[0], p[1], p[2]);
        // Ignora casi negros y casi blancos: no aportan color.
        let w = s * (1.0 - (2.0 * l - 1.0).abs());
        for i in 0..3 {
            sum[i] += p[i] as f32 * w;
        }
        weight += w;
    }
    if weight < 1.0 {
        return (122, 162, 247);            // imagen sin color: azul neutro
    }
    let avg = sum.map(|c| (c / weight) as u8);
    let (h, s, _) = to_hsl(avg[0], avg[1], avg[2]);
    if s < 0.12 {
        return (122, 162, 247);
    }
    from_hsl(h, s.clamp(0.45, 0.75), 0.68)
}
```

**Por qué no basta con hacer la media de todos los píxeles:** en una foto con fondo blanco y camiseta roja, la media sale rosa pálido casi gris. Por eso cada píxel se **pondera** según lo "colorido" que es:

- `s` es la **saturación**: cuánto color tiene (0 = gris, 1 = puro).
- `1 - |2l - 1|` vale 1 para luminosidad media y 0 para negro o blanco puros.

Un gris o un blanco pesan casi 0. Un rojo vivo pesa casi 1.

Con la media ya calculada, se pasa a **HSL** (tono, saturación, luminosidad). Se conserva el **tono** (qué color es) y se fuerzan la saturación a 45–75% y la luminosidad a 68%. Así el acento siempre se lee bien sobre fondo oscuro, aunque la foto sea muy oscura.

Las funciones `to_hsl` y `from_hsl` son las fórmulas estándar de conversión RGB ↔ HSL.

## Limitaciones y alternativas

- A 28 columnas la foto se ve **pixelada**. Es el límite físico de este método: 2 píxeles por carácter.
- Para alta resolución existen protocolos gráficos que dibujan la imagen real:
  - **Kitty graphics protocol:** kitty, WezTerm, Ghostty.
  - **Sixel:** Konsole, foot, xterm.
  - **iTerm2 inline images.**

  Serían una mejora futura, con los medios bloques como alternativa para el resto de terminales.

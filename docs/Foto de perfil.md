---
tags: [volgit, imagen, ansi, kitty]
---

# Foto de perfil

Volver a [[Volgit]] · El código está en `src/avatar.rs`

Esta es la parte con más truco de todo el proyecto. La foto se puede pintar de dos maneras:

- Con bloques de colores, que funciona en cualquier terminal pero se ve pixelado.
- Con el protocolo gráfico de kitty, que enseña la imagen de verdad. Solo en kitty, Ghostty y WezTerm.

Por defecto (`--image auto`) usa kitty si la terminal lo soporta y bloques si no. Se puede forzar con `--image kitty` o `--image blocks`.

## Modo bloques

### La idea: medios bloques

Una celda de la terminal mide más o menos el doble de alto que de ancho, así que si pinto una celda entera como si fuera un píxel, la foto sale estirada. El truco es usar el carácter `▀` (medio bloque de arriba):

```
┌───┐
│▀▀▀│ ← color del TEXTO  = píxel de arriba
│   │ ← color del FONDO  = píxel de abajo
└───┘
```

Así cada celda pinta 2 píxeles cuadrados. Una foto de 28×28 píxeles ocupa 28 columnas y 14 filas.

### Los colores con códigos ANSI

Para cambiar de color hay que mandarle a la terminal unas secuencias que empiezan por el carácter ESC (`\x1b`):

| Secuencia | Qué hace |
|---|---|
| `\x1b[38;2;R;G;Bm` | Color del texto en RGB (38 = texto, 2 = modo 24 bits) |
| `\x1b[48;2;R;G;Bm` | Color del fondo en RGB (48 = fondo) |
| `\x1b[49m` | Vuelve al fondo normal de la terminal |
| `\x1b[0m` | Quita todos los estilos |

Por ejemplo, `\x1b[38;2;255;0;0m\x1b[48;2;0;0;255m▀` pinta una celda roja arriba y azul abajo.

### Pintar una celda

```rust
fn cell(top: &Rgba<u8>, bottom: Option<&Rgba<u8>>) -> String {
    let bottom = bottom.filter(|p| visible(p));
    match (visible(top), bottom) {
        // Los dos se ven: ▀ con el de arriba como texto y el de abajo como fondo.
        (true, Some(b)) => format!(
            "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m▀",
            top[0], top[1], top[2], b[0], b[1], b[2]
        ),
        // Solo el de arriba: ▀ sobre el fondo de la terminal.
        (true, None) => format!("\x1b[49m\x1b[38;2;{};{};{}m▀", top[0], top[1], top[2]),
        // Solo el de abajo: uso ▄ (medio bloque de ABAJO) con el color de texto.
        (false, Some(b)) => format!("\x1b[49m\x1b[38;2;{};{};{}m▄", b[0], b[1], b[2]),
        // Ninguno: un espacio y a correr.
        (false, None) => "\x1b[0m ".to_string(),
    }
}
```

¿Por qué 4 casos? Por las esquinas redondeadas, que son píxeles transparentes. Si pintara un píxel transparente de negro, en una terminal con fondo gris se vería un cuadrado negro en la esquina. Así que los transparentes nunca llevan color y dejan ver el fondo, sea el que sea.

- `bottom` es un `Option` porque, si la imagen tiene un número impar de filas, la última celda no tiene píxel de abajo.
- Hacer `match` con la tupla `(visible(top), bottom)` me deja cubrir las 4 combinaciones de golpe, y el compilador no me deja olvidarme de ninguna.

### De la imagen a las líneas

```rust
fn blocks(img: &RgbaImage, cols: usize) -> Self {
    let size = cols as u32;
    // Lanczos3 conserva mejor los detalles al reducir tanto la imagen.
    let mut img = image::imageops::resize(img, size, size, FilterType::Lanczos3);
    round_corners(&mut img, size as f32 * CORNER);

    let mut lines = Vec::with_capacity(cols / 2 + 1);
    for y in (0..size).step_by(2) {           // de 2 en 2 filas, que cada celda son 2 píxeles
        let mut line = String::new();
        for x in 0..size {
            let top = img.get_pixel(x, y);
            let bottom = (y + 1 < size).then(|| img.get_pixel(x, y + 1));
            line += &cell(top, bottom);
        }
        line += "\x1b[0m";                    // reseteo al final para no manchar el texto de al lado
        lines.push(line);
    }
    Self { lines, width: cols, accent: accent(&img) }
}
```

- Lo del filtro: al pasar de 112 píxeles a 28, un filtro sencillo (vecino más cercano) coge 1 píxel de cada 4 y la cara sale llena de ruido. Lanczos3 hace una media ponderada de los vecinos y es lo más nítido que se puede sacar a ese tamaño.
- En `main.rs` pido la foto a GitHub ya a `cols × 4` píxeles con el parámetro `s=` de la URL. Así me bajo poco y el filtro todavía tiene detalle con el que trabajar.

## Modo kitty

kitty tiene un protocolo propio para meter imágenes de verdad en la terminal. Le mando un PNG y él lo dibuja en las celdas que le diga. La secuencia tiene esta pinta:

```
ESC _G <claves separadas por comas> ; <datos en base64> ESC \
```

```rust
fn kitty_escape(png: &[u8], cols: usize, rows: usize) -> String {
    let data = BASE64.encode(png);
    let chunks: Vec<&[u8]> = data.as_bytes().chunks(4096).collect();
    let mut out = String::with_capacity(data.len() + chunks.len() * 24);
    for (i, chunk) in chunks.iter().enumerate() {
        let more = u8::from(i + 1 < chunks.len());
        if i == 0 {
            out += &format!("\x1b_Ga=T,f=100,c={cols},r={rows},C=1,q=2,m={more};");
        } else {
            out += &format!("\x1b_Gm={more},q=2;");
        }
        out += std::str::from_utf8(chunk).unwrap_or_default();
        out += "\x1b\\";
    }
    out
}
```

Las claves que uso:
- `a=T`: mandar la imagen y enseñarla a la vez.
- `f=100`: lo que va es un PNG.
- `c` y `r`: cuántas columnas y filas tiene que ocupar. kitty la escala para que quepa.
- `C=1`: que no mueva el cursor. Esto es clave, porque después tengo que escribir el texto de al lado.
- `q=2`: que la terminal no conteste. Si no, su "OK" me aparecería escrito en la shell.
- `m=1` o `m=0`: si quedan más trozos o es el último.

El protocolo obliga a partir el base64 en trozos de como mucho 4096 bytes, y solo el primero lleva todas las claves. El resto solo llevan `m` (y `q`). Una foto de 460 píxeles son unos 120 trozos.

### Que salga cuadrada

Como le digo a kitty cuántas columnas y filas ocupar, tengo que calcular las filas yo para que la foto no salga estirada. Para eso le pregunto a la terminal cuánto miden sus celdas en píxeles (`term::cell_aspect()`, que usa `ioctl`). Si una celda mide 10×20, son 0.5, así que 28 columnas son 14 filas. Si no puedo preguntar, asumo 0.5, que es lo típico.

### Encajarla con el texto

El render pone la foto a la izquierda y el texto a la derecha línea a línea (ver [[Renderizado#Foto y texto en paralelo]]). Para no tener que tocar nada del render, el modo kitty devuelve las líneas igual que el de bloques: la primera lleva la secuencia de la imagen más espacios y las demás solo espacios. Esos espacios reservan el hueco para que el texto no se meta encima de la foto. Gracias a `C=1`, el cursor se queda donde empieza la imagen y los espacios lo van moviendo hasta el sitio del texto.

## Esquinas redondeadas

```rust
fn round_corners(img: &mut RgbaImage, r: f32) {
    let (w, h) = (img.width() as f32, img.height() as f32);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);   // el centro del píxel
        let cx = px.clamp(r, w - r);
        let cy = py.clamp(r, h - r);
        let dist = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
        // dist <= r - 0.5 → opaco; dist >= r + 0.5 → transparente; entre medias, gradiente.
        let coverage = (r + 0.5 - dist).clamp(0.0, 1.0);
        p[3] = (p[3] as f32 * coverage) as u8;
    }
}
```

Esto tiene más miga de lo que parece. Me imagino un rectángulo metido `r` píxeles hacia dentro desde cada borde:

```
┌──────────────┐
│ ·          · │   · = zona de esquina
│   ┌──────┐   │
│   │      │   │   el rectángulo de dentro
│   └──────┘   │
│ ·          · │
└──────────────┘
```

- `clamp` encuentra el punto de ese rectángulo más cercano al píxel. Si el píxel está en un lateral, la distancia sale menor que `r` y se queda como está. Si está en una esquina, el punto más cercano es el vértice del rectángulo.
- Si la distancia pasa de `r`, el píxel está fuera del arco y lo hago transparente (el canal alfa, `p[3]`).
- Los píxeles justo en el borde se quedan semitransparentes, según cuánto los corte el arco. Eso suaviza el borde (el antialiasing). En bloques casi no se nota, pero en kitty, con la foto a 460 píxeles, sin esto la esquina se ve dentada.

Así salen las 4 esquinas con una sola fórmula, sin un `if` por esquina. Probé con un círculo pero a 28 píxeles quedaba muy dentado, así que me quedé con un cuadrado con radio del 22% del lado (la constante `CORNER`).

## Color de acento

El color de las etiquetas y los títulos lo saco de la foto, así cada perfil tiene el suyo.

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
        return (122, 162, 247);            // foto sin color: azul neutro
    }
    let avg = sum.map(|c| (c / weight) as u8);
    let (h, s, _) = to_hsl(avg[0], avg[1], avg[2]);
    if s < 0.12 {
        return (122, 162, 247);
    }
    from_hsl(h, s.clamp(0.45, 0.75), 0.68)
}
```

No vale con hacer la media de todos los píxeles: en una foto con fondo blanco y camiseta roja, la media sale un rosa palidísimo casi gris. Por eso cada píxel cuenta más o menos según lo "colorido" que sea:

- `s` es la saturación, o sea, cuánto color tiene (0 es gris, 1 es color puro).
- `1 - |2l - 1|` vale 1 con luminosidad media y 0 para el negro o el blanco puros.

Un gris o un blanco no pesan casi nada, y un rojo vivo pesa casi 1.

Con la media hecha, paso a HSL (tono, saturación, luminosidad). Me quedo con el tono, que es "qué color es", y fuerzo la saturación entre 45 y 75% y la luminosidad al 68%. Así el acento siempre se lee bien sobre fondo oscuro aunque la foto sea muy oscura. `to_hsl` y `from_hsl` son las fórmulas de siempre para pasar de RGB a HSL y al revés.

## Pendiente

- Sixel, para que Konsole o foot también vean la foto en alta resolución.
- Dentro de tmux se podría hacer pasar la imagen con el modo *passthrough* de tmux, pero de momento ahí uso bloques.

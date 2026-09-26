---
tags: [volgit, rust, índice]
---

# Volgit

Volgit es una herramienta de terminal que estoy haciendo en Rust para consultar cosas de GitHub: la ficha de un repo, el perfil de un usuario o el de una organización, con la foto incluida. La idea salió de `onefetch`, que me gusta pero se queda corto porque solo mira el repo que tienes en local. Volgit tira de la API de GitHub, así que vale para cualquier repo o perfil público.

## Mis notas

- [[Uso]]: cómo se usa, todas las opciones y lo del token.
- [[Arquitectura]]: qué hace cada archivo y por dónde pasa un comando desde que lo escribes.
- [[API de GitHub]]: qué le pido a GitHub, los límites y cómo gestiono los errores.
- [[Foto de perfil]]: cómo pinto la foto (con bloques o con kitty) y de dónde sale el color de acento.
- [[Renderizado]]: la maquetación, la barra de lenguajes, el panel de contribuciones y la actividad.
- [[Rust en Volgit]]: mi chuleta de Rust con ejemplos sacados del propio código.

## Cómo está organizado el código

```
src/
├── main.rs     la CLI: lee argumentos, decide si es repo o usuario y lo junta todo
├── github.rs   el cliente de la API y los structs con los datos
├── avatar.rs   la foto: bloques de colores o imagen real para kitty
├── term.rs     preguntas a la terminal: ancho, tamaño de celda, si soporta imágenes
└── render.rs   todo lo que se imprime
```

## Comandos que uso todo el rato

```bash
cargo run -- @BurntSushi     # probar mientras desarrollo
cargo test                   # pasar los tests
cargo build --release        # binario optimizado en target/release/volgit
cargo install --path .       # instalarlo como comando `volgit`
```

Ojo con esto último: el `volgit` instalado no se actualiza solo. Cada vez que cambio algo tengo que volver a hacer `cargo install --path .`.

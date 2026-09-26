---
tags: [volgit, rust, índice]
---

# Volgit

Herramienta de terminal escrita en Rust que muestra la ficha de un **repositorio**, un **usuario** o una **organización** de GitHub, con su foto de perfil dibujada en la propia terminal. La idea es parecida a `onefetch`. La diferencia es que `onefetch` solo lee el repo local, mientras que Volgit consulta la API de GitHub y sirve para cualquier repo o perfil público.

## Notas

| Nota | Qué cuenta |
|---|---|
| [[Uso]] | Comandos, opciones y el token de GitHub |
| [[Arquitectura]] | Qué hace cada archivo y cómo fluye un comando de principio a fin |
| [[API de GitHub]] | Qué endpoints se consultan, límites de peticiones y cómo se gestionan los errores |
| [[Foto de perfil]] | Cómo se dibuja una imagen con caracteres y cómo se saca el color de acento |
| [[Renderizado]] | Maquetación en columnas, barras de lenguajes y agrupación de la actividad |
| [[Rust en Volgit]] | Los conceptos de Rust que aparecen en el código, explicados con ejemplos del proyecto |

## Mapa rápido del código

```
src/
├── main.rs     CLI: lee argumentos, decide si es repo o usuario, orquesta todo
├── github.rs   Cliente HTTP de la API de GitHub y structs de datos
├── avatar.rs   Imagen → caracteres de colores + color de acento
└── render.rs   Todo lo que se imprime en pantalla
```

## Comandos de desarrollo

```bash
cargo run -- @BurntSushi     # ejecutar en modo desarrollo
cargo test                   # tests
cargo build --release        # binario optimizado en target/release/volgit
cargo install --path .       # instalarlo como comando `volgit`
```

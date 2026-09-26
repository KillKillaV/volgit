---
tags: [volgit, uso]
---

# Uso

Volver a [[Volgit]].

## Qué se le puede pasar

Volgit recibe un único argumento, el **objetivo**, y deduce de qué se trata:

| Escribes | Volgit entiende |
|---|---|
| `sharkdp/bat` | Repositorio |
| `https://github.com/sharkdp/bat` o `.git` | Repositorio |
| `git@github.com:sharkdp/bat.git` | Repositorio (formato SSH) |
| `@BurntSushi` | Usuario |
| `rust-lang` (sin `/`) | Usuario u organización |
| `https://github.com/torvalds` | Usuario |
| *(nada)* | El repo del directorio actual, leyendo `git remote get-url origin` |
| `help` | La ayuda. Para consultar un usuario llamado "help", escribe `@help` |

La lógica que lo decide está explicada en [[Arquitectura#Decidir si es repo o usuario]].

## Opciones

| Opción | Efecto |
|---|---|
| `-t, --top N` o `--top all` | Cuántos contribuidores o repos mostrar (por defecto 5). `all` muestra todos: hasta 500 repos sin forks y 100 contribuidores |
| `-p, --panel` | Panel de contribuciones del último año (los cuadraditos verdes). Solo usuarios; requiere token |
| `--json` | Vuelca todos los datos en JSON, útil para `jq` y scripts |
| `--no-color` | Sin colores y sin foto |
| `--no-avatar` | Sin foto |
| `--avatar-size N` | Ancho de la foto en columnas, de 8 a 80 (por defecto 28) |
| `--token TOKEN` | Token de GitHub. Si no se indica, se lee `GITHUB_TOKEN` |
| `-h, --help` / `help` | Ayuda con ejemplos |
| `-V, --version` | Versión |

La foto se omite automáticamente cuando la salida no va a una terminal, por ejemplo `volgit x/y > archivo.txt`. Así el archivo no se llena de códigos de color.

## El token de GitHub

| | Peticiones por hora |
|---|---|
| Sin token | 60 por IP |
| Con token | 5000 |

Un perfil gasta hasta 7 peticiones y un repo, 5. Sin token el límite se agota rápido. Ver [[API de GitHub#Límites]].

Para crear uno:
1. Ve a https://github.com/settings/personal-access-tokens y crea uno de tipo *fine-grained* **sin ningún permiso**. Para leer datos públicos no hace falta ninguno.
2. Añádelo a `~/.bashrc`:
   ```bash
   export GITHUB_TOKEN=github_pat_...
   ```
3. Nunca lo metas dentro del proyecto, para que no acabe en git.

## Requisitos de la terminal

- **Color verdadero (24 bits)** para ver bien la foto y los colores. La mayoría de terminales modernas lo soportan y lo anuncian con `COLORTERM=truecolor`. Si esa variable no está, los colores de texto se aproximan a la paleta básica.
- Una fuente con los caracteres `▀ ▄ ━ ● ★`. Cualquier fuente moderna para programar los tiene.

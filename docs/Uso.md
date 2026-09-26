---
tags: [volgit, uso]
---

# Uso

Volver a [[Volgit]]

## Qué le puedo pasar

Volgit recibe una sola cosa y él solito deduce qué es:

```bash
volgit sharkdp/bat                        # un repo
volgit https://github.com/sharkdp/bat     # también con URL, con o sin .git
volgit git@github.com:sharkdp/bat.git     # o en formato SSH
volgit @BurntSushi                        # un usuario
volgit rust-lang                          # sin barra también lo toma como usuario u organización
volgit https://github.com/torvalds        # perfil por URL
volgit                                    # el repo de la carpeta en la que estoy (su remote origin)
volgit help                               # la ayuda
```

Lo de `help` tiene una pega tonta: si algún día quiero ver a un usuario que se llame literalmente "help", tengo que escribir `@help`.

Cómo decide si es repo o usuario lo tengo explicado en [[Arquitectura#Decidir si es repo o usuario]].

## Opciones

| Opción | Qué hace |
|---|---|
| `-t N` / `--top N` | Cuántos repos o contribuidores enseña (5 por defecto) |
| `--top all` | Todos. Como mucho 500 repos (sin forks) y 100 contribuidores |
| `-r` / `--repos` | Los repos destacados del perfil |
| `-a` / `--activity` | La actividad reciente del perfil |
| `-p` / `--panel` | El panel de cuadraditos verdes de contribuciones. Solo usuarios y necesita token |
| `--image MODO` | Cómo pinta la foto: `auto`, `kitty` o `blocks` |
| `--avatar-size N` | Ancho de la foto en columnas, de 8 a 80 (28 por defecto) |
| `--no-avatar` | Sin foto |
| `--no-color` | Sin colores (y sin foto) |
| `--json` | Suelta todos los datos en JSON, para usarlo con `jq` o en scripts |
| `--token TOKEN` | El token. Si no lo pongo, lo coge de `GITHUB_TOKEN` |
| `-V` | La versión |

Por defecto el perfil sale sin repos, sin actividad y sin panel: solo los datos y los lenguajes. Lo demás lo pido cuando lo quiero, y se pueden juntar: `volgit @BurntSushi -rap` lo saca todo.

Sobre `--image`: en `auto` usa la imagen de verdad si detecta kitty (o Ghostty o WezTerm) y los bloques de colores en cualquier otra terminal. Dentro de tmux siempre bloques, porque tmux no deja pasar las imágenes. Más detalles en [[Foto de perfil]].

Si mando la salida a un archivo (`volgit x/y > algo.txt`), la foto no sale. Así el archivo no se llena de basura de códigos de color.

## El token

Sin token GitHub solo me deja hacer 60 peticiones por hora, y un perfil ya se come hasta 7. Con token son 5000, que no se acaban nunca. Encima el panel de contribuciones directamente no funciona sin token.

Cómo sacarlo:
1. Entro en https://github.com/settings/personal-access-tokens y creo uno *fine-grained* sin marcar ningún permiso. Para leer cosas públicas no le hace falta nada.
2. Lo meto en `~/.bashrc`:
   ```bash
   export GITHUB_TOKEN=github_pat_...
   ```
3. Y nunca lo pongo dentro del proyecto, que acaba en git sin darme cuenta.

Si se me agota el límite, Volgit me dice a qué hora se reinicia. Más en [[API de GitHub#Límites]].

## Qué necesita la terminal

- Color de 24 bits. Casi todas las terminales modernas lo tienen y lo avisan con `COLORTERM=truecolor`. Si no está esa variable, los colores del texto se ven con la paleta básica.
- Una fuente que tenga `▀ ▄ ━ ● ★ ■`. Cualquier fuente de programar sirve.
- Para ver la foto en alta resolución, kitty, Ghostty o WezTerm.

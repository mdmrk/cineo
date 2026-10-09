page-home = Inicio
page-discover = Descubrir
page-search = Buscar
page-library = Biblioteca
page-addons = Complementos
page-settings = Ajustes

app-version = Cineo v{ $version }
loading-addons = Cargando complementos…
back = Atrás
cancel = Cancelar
dismiss = Cerrar
remove = Quitar
retry = Reintentar
clear-field = Borrar
play = Reproducir
see-all = Ver todo
scroll-left = Desplazar a la izquierda
scroll-right = Desplazar a la derecha

p2p-title = Streaming entre pares (P2P)
p2p-accept = Aceptar y reproducir
p2p-notice = Las transmisiones torrent vienen de los ordenadores de otras personas. Mientras se reproduce una, tu dirección IP es visible para los pares y trackers a los que se conecta, y Cineo sube a esos pares las partes que ya ha descargado. Los datos descargados se guardan en una caché local. Puedes desactivar el streaming entre pares en Ajustes.

continue-watching = Seguir viendo
home-empty = No hay complementos instalados. Los catálogos aparecen aquí cuando añades uno con la URL de su manifiesto.
open-addons = Abrir Complementos
discover-empty = Ningún complemento instalado tiene un catálogo para explorar.
all-genres = Todos los géneros
search-hint = Buscar películas y series
search-help = Los resultados vienen de todos los complementos instalados que admiten búsqueda.
search-unsupported = Ningún complemento instalado admite búsqueda.
favourites = Favoritos
recently-played = Reproducido recientemente
library-empty = Todo lo que reproduzcas aparece aquí.
count-titles =
    { $count ->
        [one] { $count } título
       *[other] { $count } títulos
    }
catalog-failed = No se pudo cargar:
catalog-empty = No hay nada en este catálogo.
catalog-title = { $name } · { $kind }
type-movie = Películas
type-series = Series
type-channel = Canales
type-tv = TV

favourite = Favorito
favourited = En favoritos
favourite-add = Añadir a favoritos
favourite-remove = Quitar de favoritos
remove-continue = Quitar de Seguir viendo
remove-library = Quitar de la biblioteca

addons-dek = Cuando dos complementos devuelven el mismo elemento, gana el que está más arriba en la lista.
addons-install = Instalar un complemento
install = Instalar
installing = Instalando…
installed-addon = { $name } instalado
addons-installed = Instalados
addons-none = Aún no hay nada instalado.
count-addons =
    { $count ->
        [one] { $count } complemento
       *[other] { $count } complementos
    }
badge-adult = Contenido adulto
badge-p2p = Entre pares
badge-configure = Requiere configuración
badge-unavailable = No se pudo cargar
move-up = Subir
move-down = Bajar
addon-unnamed = Complemento

directed-by = Dirigida por
genres = Géneros
cast = Reparto
rating-imdb = { $rating } IMDb
episodes = Episodios
season = Temporada { $number }
specials = Especiales
where-to-watch = Dónde ver
streams-unavailable = Ningún complemento instalado ofrece transmisiones para este elemento.
streams-search = Buscar transmisiones
streams-all = Todas
sort-by = Ordenar por
sort-addon-order = Orden de complementos
sort-quality = Calidad
sort-seeders = Semillas
sort-size = Tamaño
count-streams =
    { $count ->
        [one] { $count } transmisión
       *[other] { $count } transmisiones
    }
streams-empty = No hay transmisiones
streams-no-match = Ninguna transmisión coincide
streams-hidden =
    { $count ->
        [one] { $count } transmisión torrent oculta: el streaming entre pares está desactivado en Ajustes
       *[other] { $count } transmisiones torrent ocultas: el streaming entre pares está desactivado en Ajustes
    }
source-http = HTTP
source-url = URL
source-youtube = YouTube
source-torrent = Torrent
source-external = Externa
source-archive = Archivo
source-usenet = Usenet

problem-already-installed = Este complemento ya está instalado
problem-invalid-url = URL de complemento no válida: { $detail }
problem-no-meta = Ningún complemento instalado ofrece detalles de este elemento
problem-unsupported-filter = Este catálogo no admite ese filtro
notice-addon-unavailable = No se pudo cargar un complemento: { $detail }
notice-unsupported-scheme = Esquema de transmisión no compatible: `{ $scheme }`
notice-torrents-off = Las transmisiones torrent están desactivadas en Ajustes
notice-unsupported-source = Las transmisiones de tipo { $kind } aún no son compatibles
notice-torrent-failed = No se pudo reproducir el torrent: { $detail }
notice-pick-another = No se pudo reproducir la última transmisión, elige otra. { $reason }
notice-engine-address = El motor de torrents devolvió una dirección inesperada
notice-subtitle-failed = No se pudo cargar el subtítulo: { $detail }
notice-save-failed = No se pudieron guardar los cambios: { $detail }
notice-store-closed = Los cambios ya no se pueden guardar

pause = Pausa
seek-back = Retroceder { $seconds } segundos
seek-forward = Avanzar { $seconds } segundos
fullscreen = Pantalla completa
exit-fullscreen = Salir de pantalla completa
player-subtitles = Subtítulos
player-audio = Audio
subtitle-style-menu = Estilo y retraso
player-stats = Estadísticas de streaming
stats-download = Descarga
stats-peers = Pares
stats-downloaded = Descargado
stats-of = { $done } de { $total }
stats-network = Red
stats-buffered = En búfer
stats-resolution = Resolución
stats-video = Vídeo
stats-decoder = Decodificador
stats-software = Software
stats-dropped = Fotogramas perdidos
mute = Silenciar
unmute = Activar sonido
position = Posición
volume = Volumen
subtitles-off = Desactivados
no-audio-tracks = No hay pistas de audio
track = Pista { $number }
delay = Retraso
delay-help = Solo este vídeo.
subtitles-earlier = Subtítulos { $amount } antes
subtitles-later = Subtítulos { $amount } después
delay-reset = Retraso de subtítulos { $delay }, restablecer

section-interface = Interfaz
section-player = Reproductor
section-languages = Idiomas
section-subtitles = Subtítulos
section-audio = Audio
section-torrents = Torrents
section-data = Datos
section-keyboard = Teclado
section-about = Acerca de

ui-language = Idioma
ui-language-help = El idioma de los menús y mensajes de Cineo.
ui-language-system = Sistema ({ $language })
interface-size = Tamaño de la interfaz
interface-size-help = Hace más grandes o más pequeños el texto, los pósteres y los controles.
start-page = Página de inicio
start-page-help = Se muestra al abrir Cineo.

hardware-decoding = Decodificación por hardware
hardware-decoding-help = Deja que la tarjeta gráfica decodifique el vídeo, lo que ahorra energía. Desactívala si los vídeos muestran artefactos o una imagen negra.
seek-step = Salto
seek-step-help = Cuánto saltan ←/→ y los botones de salto.
short-seek-step = Salto corto
short-seek-step-help = Cuánto salta Mayús+←/→.
hide-controls = Ocultar controles tras
hide-controls-help = Sin usar el ratón ni el teclado durante la reproducción.
escape-fullscreen = Esc sale primero de pantalla completa
escape-fullscreen-help = Si está desactivado, Esc cierra el reproductor directamente.
pause-minimized = Pausar al minimizar
remember-volume = Recordar el volumen
remember-volume-help = Cada vídeo empieza con el volumen con el que terminó el anterior.
seconds =
    { $count ->
        [one] { $count } segundo
       *[other] { $count } segundos
    }

audio-language = Idioma del audio
audio-language-help = Se reproduce la pista del archivo en este idioma; si no hay, su pista predeterminada.
audio-language-second = Segundo idioma del audio
audio-language-second-help = Se usa cuando el archivo no tiene audio en el primero.
subtitle-language = Idioma de los subtítulos
subtitle-language-help = Se activan al empezar un vídeo: del archivo si los tiene y, si no, de un complemento de subtítulos.
subtitle-language-second = Segundo idioma de los subtítulos
subtitle-language-second-help = Se usa cuando no hay subtítulos en el primero.
language-none = Ninguno

subtitle-size = Tamaño
subtitle-font = Fuente
subtitle-bold = Negrita
subtitle-color = Color del texto
subtitle-opacity = Opacidad del texto
subtitle-outline = Contorno
subtitle-outline-help = No se dibuja cuando hay fondo.
subtitle-background = Fondo
subtitle-raise = Elevar desde abajo
subtitle-raise-help = Porcentaje de la altura de la imagen.
subtitle-keep-styles = Mantener el aspecto de los subtítulos con estilo
subtitle-keep-styles-help = Los subtítulos con estilo (ASS), habituales en el anime, traen sus propias fuentes, colores y posiciones. Si está desactivado, los ajustes de arriba los sustituyen.
subtitle-preview = Así se ven los subtítulos.
subtitle-preview-badge = VISTA PREVIA
font-sans = Sans serif
font-serif = Serif
font-mono = Monoespaciada
color-white = Blanco
color-yellow = Amarillo
color-cyan = Cian
color-green = Verde
outline-black = Negro
outline-gray = Gris
outline-none = Sin contorno
background-none = Sin fondo
background-black = Caja negra
background-gray = Caja gris

audio-output = Salida de audio
audio-output-help = Estéreo convierte el sonido envolvente a dos altavoces o auriculares.
output-auto = Automática (envolvente)
output-stereo = Estéreo
passthrough = Passthrough
passthrough-help = Envía el audio Dolby y DTS sin decodificar a un receptor por HDMI o S/PDIF. Déjalo desactivado salvo que tu receptor los decodifique, o puede que no oigas nada.

p2p-enabled = Mostrar y reproducir transmisiones torrent
torrent-upload = Subir a otros pares
torrent-upload-help = Si está desactivado, Cineo solo descarga. Algunos pares envían entonces menos, así que los torrents pueden ir más lentos. Tu dirección IP sigue siendo visible para los pares.
download-limit = Límite de descarga
upload-limit = Límite de subida
next-torrent = Se aplica desde el próximo torrent que reproduzcas.
no-limit = Sin límite
peer-limit = Pares por torrent
peer-limit-help = Más pares puede ser más rápido, pero usa más conexiones.
dht = Buscar pares mediante la DHT
dht-help = Encuentra pares sin trackers. Desactivarla significa menos pares para muchos torrents.
private-network = Permitir direcciones de la red local
private-network-help = Permite acceder a complementos, imágenes y pares torrent en 127.0.0.1 o en tu red doméstica, para complementos alojados por ti. Desactivado es más seguro. Se aplica al reiniciar Cineo.

binge-watching = Reproducir el siguiente episodio automáticamente
binge-watching-help = Al terminar un episodio, se reproduce el siguiente del mismo addon o se abren sus fuentes.
next-notice = Aviso del siguiente episodio
next-notice-help = Cuánto antes del final se ofrece el siguiente episodio.
next-notice-off = Desactivado
next-episode = Siguiente episodio
next-episode-in = Siguiente episodio en { $seconds } s

watched-at = Contar como visto al
watched-at-help = Los vídeos vistos empiezan de nuevo y salen de Seguir viendo.
watched-percent = { $percent } % reproducido
clear-history = Borrar el historial
clear-history-help = Vacía la Biblioteca y Seguir viendo.
clear-ellipsis = Borrar…
reset-settings = Restablecer todos los ajustes
reset-settings-help = Todos los ajustes de esta página vuelven a su valor predeterminado.
reset-ellipsis = Restablecer…
reset-confirm-title = ¿Restablecer todos los ajustes?
reset-confirm-text = Todos los ajustes vuelven a su valor predeterminado. Tus complementos y tu biblioteca se conservan.
reset = Restablecer
clear-confirm-title = ¿Borrar el historial?
clear-confirm-text = Todos los elementos y su progreso salen de la Biblioteca. No se puede deshacer.
clear = Borrar

key-or = { $a }  o  { $b }
key-space = Espacio
key-shift-wheel = Mayús+rueda
key-mouse-back = botón atrás del ratón
shortcut-search = Buscar
shortcut-switch-page = Cambiar de página
shortcut-leave-detail = Salir de una página de detalle
shortcut-scroll-row = Desplazar una fila de lado
shortcut-play-pause = Reproducir o pausar
shortcut-seek = Saltar según el salto
shortcut-short-seek = Saltar según el salto corto
shortcut-volume = Volumen  ·  silencio
shortcut-fullscreen = Pantalla completa

about-license = Cineo es software libre bajo la licencia MIT.
about-credits = Hecho con Rust, egui, mpv y librqbit.

language-ara = Árabe
language-bul = Búlgaro
language-cat = Catalán
language-chi = Chino
language-hrv = Croata
language-cze = Checo
language-dan = Danés
language-dut = Neerlandés
language-eng = Inglés
language-fin = Finés
language-fre = Francés
language-ger = Alemán
language-gre = Griego
language-heb = Hebreo
language-hin = Hindi
language-hun = Húngaro
language-ind = Indonesio
language-ita = Italiano
language-jpn = Japonés
language-kor = Coreano
language-nor = Noruego
language-per = Persa
language-pol = Polaco
language-por = Portugués
language-rum = Rumano
language-rus = Ruso
language-srp = Serbio
language-spa = Español
language-swe = Sueco
language-tha = Tailandés
language-tur = Turco
language-ukr = Ucraniano
language-vie = Vietnamita

notice-invalid-link = Cineo no puede abrir ese enlace
link-install-title = ¿Instalar este addon?
link-install-notice = Un enlace pide instalar el addon de abajo. Instala solo addons de confianza.
links-register = Abrir enlaces stremio:// y cineo://
links-register-help = Los enlaces a addons y páginas se abren en Cineo. Stremio deja de recibir los enlaces stremio://.
links-register-button = Usar Cineo
links-registered = Hecho
links-register-failed = Error: { $detail }
configure = Configurar
problem-configuration-required = Este addon debe configurarse antes de instalarlo. Configúralo e instala el enlace que te dé.

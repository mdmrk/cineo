page-home = Home
page-discover = Discover
page-search = Search
page-library = Library
page-addons = Addons
page-settings = Settings

app-version = Cineo v{ $version }
loading-addons = Loading addons…
back = Back
cancel = Cancel
dismiss = Dismiss
remove = Remove
retry = Retry
clear-field = Clear
play = Play
see-all = See all
scroll-left = Scroll left
scroll-right = Scroll right

p2p-title = Peer-to-peer streaming
p2p-accept = Accept and play
p2p-notice = Torrent streams come from other people's computers. While one plays, your IP address is visible to the peers and trackers it connects to, and Cineo uploads the parts it has already downloaded to those peers. Downloaded data is kept in a local cache. You can turn peer-to-peer streaming off in Settings.

continue-watching = Continue watching
home-empty = No addons installed. Catalogs show up here once you add one by its manifest URL.
open-addons = Open Addons
discover-empty = No installed addon has a browsable catalog.
all-genres = All genres
search-hint = Search movies and series
search-help = Results come from every installed addon that supports search.
search-unsupported = No installed addon supports search.
favourites = Favourites
recently-played = Recently played
library-empty = Anything you play shows up here.
count-titles =
    { $count ->
        [one] { $count } title
       *[other] { $count } titles
    }
catalog-failed = Could not load:
catalog-empty = Nothing in this catalog.
catalog-title = { $name } { $kind }
type-movie = Movies
type-series = Series
type-channel = Channels
type-tv = TV

favourite = Favourite
favourited = Favourited
favourite-add = Add to favourites
favourite-remove = Remove from favourites
remove-continue = Remove from continue watching
remove-library = Remove from library

addons-dek = When two addons return the same item, the one higher in the list wins.
addons-install = Install an addon
install = Install
installing = Installing…
installed-addon = Installed { $name }
addons-installed = Installed
addons-none = Nothing installed yet.
count-addons =
    { $count ->
        [one] { $count } addon
       *[other] { $count } addons
    }
badge-adult = Adult content
badge-p2p = Peer-to-peer
badge-configure = Needs configuration
badge-unavailable = Could not load
move-up = Move up
move-down = Move down
addon-unnamed = Addon

directed-by = Directed by
genres = Genres
cast = Cast
rating-imdb = { $rating } IMDb
episodes = Episodes
season = Season { $number }
specials = Specials
where-to-watch = Where to watch
streams-unavailable = No installed addon provides streams for this item.
streams-search = Search streams
streams-all = All
sort-by = Sort by
sort-addon-order = Addon order
sort-quality = Quality
sort-seeders = Seeders
sort-size = Size
count-streams =
    { $count ->
        [one] { $count } stream
       *[other] { $count } streams
    }
streams-empty = No streams
streams-no-match = No matching streams
streams-hidden =
    { $count ->
        [one] { $count } torrent stream hidden: peer-to-peer is off in Settings
       *[other] { $count } torrent streams hidden: peer-to-peer is off in Settings
    }
source-http = HTTP
source-url = URL
source-youtube = YouTube
source-torrent = Torrent
source-external = External
source-archive = Archive
source-usenet = Usenet

problem-already-installed = This addon is already installed
problem-invalid-url = Invalid addon URL: { $detail }
problem-no-meta = No installed addon provides details for this item
problem-unsupported-filter = This catalog does not support that filter
notice-addon-unavailable = An addon could not be loaded: { $detail }
notice-unsupported-scheme = Unsupported stream scheme `{ $scheme }`
notice-torrents-off = Torrent streams are turned off in Settings
notice-unsupported-source = { $kind } streams are not supported yet
notice-torrent-failed = The torrent could not be played: { $detail }
notice-pick-another = The last stream could not be played, pick another one. { $reason }
notice-engine-address = The torrent engine returned an unexpected address
notice-subtitle-failed = Could not load the subtitle: { $detail }
notice-save-failed = Could not save changes: { $detail }
notice-store-closed = Changes can no longer be saved

pause = Pause
seek-back = Seek back { $seconds } seconds
seek-forward = Seek forward { $seconds } seconds
fullscreen = Fullscreen
exit-fullscreen = Exit fullscreen
player-subtitles = Subtitles
player-audio = Audio
mute = Mute
unmute = Unmute
position = Position
volume = Volume
subtitles-off = Off
no-audio-tracks = No audio tracks
track = Track { $number }
delay = Delay
delay-help = This video only.
subtitles-earlier = Subtitles { $amount } earlier
subtitles-later = Subtitles { $amount } later
delay-reset = Subtitle delay { $delay }, reset

section-interface = Interface
section-player = Player
section-languages = Languages
section-subtitles = Subtitles
section-audio = Audio
section-torrents = Torrents
section-data = Data
section-keyboard = Keyboard
section-about = About

ui-language = Language
ui-language-help = The language of Cineo's menus and messages.
ui-language-system = System ({ $language })
interface-size = Interface size
interface-size-help = Makes text, posters and controls larger or smaller.
start-page = Start page
start-page-help = Shown when Cineo opens.

hardware-decoding = Hardware decoding
hardware-decoding-help = Lets the graphics card decode video, which saves power. Turn it off if videos show artifacts or a black picture.
seek-step = Seek step
seek-step-help = How far ←/→ and the seek buttons jump.
short-seek-step = Short seek step
short-seek-step-help = How far Shift+←/→ jump.
hide-controls = Hide controls after
hide-controls-help = Without mouse or key input while playing.
escape-fullscreen = Esc leaves fullscreen first
escape-fullscreen-help = When off, Esc leaves the player at once.
pause-minimized = Pause when minimized
remember-volume = Remember volume
remember-volume-help = Each video starts at the volume the last one ended with.
seconds =
    { $count ->
        [one] { $count } second
       *[other] { $count } seconds
    }

audio-language = Audio language
audio-language-help = The file's track in this language plays; otherwise its default track.
audio-language-second = Second audio language
audio-language-second-help = Used when the file has no audio in the first.
subtitle-language = Subtitle language
subtitle-language-help = Turned on when a video starts: from the file if it has them, otherwise from a subtitles addon.
subtitle-language-second = Second subtitle language
subtitle-language-second-help = Used when no subtitle is in the first.
language-none = None

subtitle-size = Size
subtitle-font = Font
subtitle-bold = Bold
subtitle-color = Text color
subtitle-opacity = Text opacity
subtitle-outline = Outline
subtitle-outline-help = Not drawn when there is a background.
subtitle-background = Background
subtitle-raise = Raise from the bottom
subtitle-raise-help = Percent of the picture's height.
subtitle-keep-styles = Keep the look of styled subtitles
subtitle-keep-styles-help = Styled (ASS) subtitles, common for anime, bring their own fonts, colors and positions. When off, the settings above replace them.
subtitle-preview = Subtitles look like this.
subtitle-preview-badge = PREVIEW
font-sans = Sans-serif
font-serif = Serif
font-mono = Monospace
color-white = White
color-yellow = Yellow
color-cyan = Cyan
color-green = Green
outline-black = Black
outline-gray = Gray
outline-none = No outline
background-none = No background
background-black = Black box
background-gray = Gray box

audio-output = Audio output
audio-output-help = Stereo mixes surround sound down to two speakers or headphones.
output-auto = Automatic (surround)
output-stereo = Stereo
passthrough = Passthrough
passthrough-help = Sends Dolby and DTS audio undecoded to a receiver over HDMI or S/PDIF. Leave off unless your receiver decodes them, or you may hear silence.

p2p-enabled = Show and play torrent streams
torrent-upload = Upload to other peers
torrent-upload-help = When off, Cineo only downloads. Some peers then send less, so torrents can be slower. Your IP address is still visible to peers.
download-limit = Download limit
upload-limit = Upload limit
next-torrent = Applies from the next torrent you play.
no-limit = No limit
peer-limit = Peers per torrent
peer-limit-help = More peers can be faster but use more connections.
dht = Find peers through the DHT
dht-help = Finds peers without trackers. Off means fewer peers for many torrents.
private-network = Allow local network addresses
private-network-help = Lets addons, images and torrent peers on 127.0.0.1 or your home network be reached, for self-hosted addons. Off is safer. Takes effect after restarting Cineo.

binge-watching = Play the next episode automatically
binge-watching-help = When an episode ends, the next one plays from the same addon, or its streams open.
next-notice = Next episode notice
next-notice-help = How long before the end the next episode is offered.
next-notice-off = Off
next-episode = Next episode
next-episode-in = Next episode in { $seconds } s

watched-at = Count as watched at
watched-at-help = Watched videos start over and leave Continue Watching.
watched-percent = { $percent } % played
clear-history = Clear watch history
clear-history-help = Empties the Library and Continue Watching.
clear-ellipsis = Clear…
reset-settings = Reset all settings
reset-settings-help = Every setting on this page goes back to its default.
reset-ellipsis = Reset…
reset-confirm-title = Reset all settings?
reset-confirm-text = Every setting goes back to its default. Your addons and library are kept.
reset = Reset
clear-confirm-title = Clear watch history?
clear-confirm-text = Every item and its progress leaves the Library. This cannot be undone.
clear = Clear

key-or = { $a }  or  { $b }
key-space = Space
key-shift-wheel = Shift+wheel
key-mouse-back = mouse back
shortcut-search = Search
shortcut-switch-page = Switch page
shortcut-leave-detail = Leave a detail page
shortcut-scroll-row = Scroll a row sideways
shortcut-play-pause = Play or pause
shortcut-seek = Seek by the seek step
shortcut-short-seek = Seek by the short seek step
shortcut-volume = Volume  ·  mute
shortcut-fullscreen = Fullscreen

about-license = Cineo is free software under the MIT License.
about-credits = Made with Rust, egui, mpv and librqbit.

language-ara = Arabic
language-bul = Bulgarian
language-cat = Catalan
language-chi = Chinese
language-hrv = Croatian
language-cze = Czech
language-dan = Danish
language-dut = Dutch
language-eng = English
language-fin = Finnish
language-fre = French
language-ger = German
language-gre = Greek
language-heb = Hebrew
language-hin = Hindi
language-hun = Hungarian
language-ind = Indonesian
language-ita = Italian
language-jpn = Japanese
language-kor = Korean
language-nor = Norwegian
language-per = Persian
language-pol = Polish
language-por = Portuguese
language-rum = Romanian
language-rus = Russian
language-srp = Serbian
language-spa = Spanish
language-swe = Swedish
language-tha = Thai
language-tur = Turkish
language-ukr = Ukrainian
language-vie = Vietnamese

notice-invalid-link = That link cannot be opened by Cineo
link-install-title = Install this addon?
link-install-notice = A link asks to install the addon below. Only install addons you trust.
links-register = Open stremio:// and cineo:// links
links-register-help = Links to addons and pages open in Cineo. Stremio stops receiving stremio:// links.
links-register-button = Use Cineo
links-registered = Done
links-register-failed = Failed: { $detail }
configure = Configure
problem-configuration-required = This addon must be configured before it can be installed. Configure it, then install the link it gives you.

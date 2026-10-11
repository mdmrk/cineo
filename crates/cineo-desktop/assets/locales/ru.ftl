page-home = Главная
page-discover = Обзор
page-search = Поиск
page-library = Библиотека
page-addons = Аддоны
page-settings = Настройки

app-version = Cineo v{ $version }
loading-addons = Загрузка аддонов…
back = Назад
cancel = Отмена
dismiss = Закрыть
remove = Удалить
retry = Повторить
clear-field = Очистить
play = Смотреть
see-all = Все
scroll-left = Прокрутить влево
scroll-right = Прокрутить вправо

p2p-title = Пиринговая трансляция
p2p-accept = Принять и смотреть
p2p-notice = Торрент-потоки поступают с компьютеров других людей. Во время просмотра ваш IP-адрес виден пирам и трекерам, к которым идёт подключение, а Cineo раздаёт этим пирам уже загруженные части. Загруженные данные хранятся в локальном кеше. Пиринговую трансляцию можно отключить в настройках.

continue-watching = Продолжить просмотр
home-empty = Аддоны не установлены. Каталоги появятся здесь, когда вы добавите аддон по URL его манифеста.
open-addons = Открыть аддоны
discover-empty = Ни у одного установленного аддона нет каталога для просмотра.
all-genres = Все жанры
search-hint = Поиск фильмов и сериалов
search-help = Результаты приходят от всех установленных аддонов, поддерживающих поиск.
search-unsupported = Ни один установленный аддон не поддерживает поиск.
favourites = Избранное
recently-played = Недавно просмотренные
library-empty = Здесь появится всё, что вы смотрите.
count-titles =
    { $count ->
        [one] { $count } название
        [few] { $count } названия
        [many] { $count } названий
       *[other] { $count } названия
    }
catalog-failed = Не удалось загрузить:
catalog-empty = В этом каталоге ничего нет.
catalog-title = { $name } { $kind }
type-movie = Фильмы
type-series = Сериалы
type-channel = Каналы
type-tv = ТВ

favourite = В избранное
favourited = В избранном
favourite-add = Добавить в избранное
favourite-remove = Удалить из избранного
remove-continue = Убрать из «Продолжить просмотр»
remove-library = Удалить из библиотеки

addons-dek = Если два аддона возвращают один и тот же элемент, побеждает тот, что выше в списке.
addons-install = Установить аддон
install = Установить
installing = Установка…
installed-addon = Установлен { $name }
addons-installed = Установленные
addons-none = Пока ничего не установлено.
addons-discover = Обзор аддонов
addons-discover-note = Аддоны из этих списков созданы сторонними разработчиками. Устанавливайте только те, которым доверяете.
addons-list-empty = Этот список пуст.
addon-installed = Установлен
count-addons =
    { $count ->
        [one] { $count } аддон
        [few] { $count } аддона
        [many] { $count } аддонов
       *[other] { $count } аддона
    }
badge-adult = Для взрослых
badge-p2p = P2P
badge-configure = Требует настройки
badge-unavailable = Не загружен
move-up = Выше
move-down = Ниже
addon-unnamed = Аддон

directed-by = Режиссёр
genres = Жанры
cast = В ролях
rating-imdb = { $rating } IMDb
episodes = Эпизоды
season = Сезон { $number }
specials = Спецвыпуски
where-to-watch = Где смотреть
streams-unavailable = Ни один установленный аддон не предоставляет потоки для этого элемента.
streams-search = Поиск потоков
streams-all = Все
sort-by = Сортировка
sort-addon-order = Порядок аддонов
sort-quality = Качество
sort-seeders = Сиды
sort-size = Размер
count-streams =
    { $count ->
        [one] { $count } поток
        [few] { $count } потока
        [many] { $count } потоков
       *[other] { $count } потока
    }
streams-empty = Нет потоков
streams-no-match = Нет подходящих потоков
streams-hidden =
    { $count ->
        [one] Скрыт { $count } торрент-поток: P2P отключён в настройках
        [few] Скрыто { $count } торрент-потока: P2P отключён в настройках
        [many] Скрыто { $count } торрент-потоков: P2P отключён в настройках
       *[other] Скрыто { $count } торрент-потока: P2P отключён в настройках
    }
source-http = HTTP
source-url = URL
source-youtube = YouTube
source-torrent = Торрент
source-external = Внешний
source-archive = Архив
source-usenet = Usenet

problem-already-installed = Этот аддон уже установлен
problem-invalid-url = Неверный URL аддона: { $detail }
problem-no-meta = Ни один установленный аддон не предоставляет сведения об этом элементе
problem-unsupported-filter = Этот каталог не поддерживает такой фильтр
notice-addon-unavailable = Не удалось загрузить аддон: { $detail }
notice-unsupported-scheme = Неподдерживаемая схема потока `{ $scheme }`
notice-torrents-off = Торрент-потоки отключены в настройках
notice-unsupported-source = Потоки типа «{ $kind }» пока не поддерживаются
notice-torrent-failed = Не удалось воспроизвести торрент: { $detail }
notice-pick-another = Не удалось воспроизвести последний поток, выберите другой. { $reason }
notice-engine-address = Торрент-движок вернул неожиданный адрес
notice-subtitle-failed = Не удалось загрузить субтитры: { $detail }
notice-save-failed = Не удалось сохранить изменения: { $detail }
notice-store-closed = Изменения больше не могут быть сохранены

pause = Пауза
seek-back = Назад на { $seconds } с
seek-forward = Вперёд на { $seconds } с
fullscreen = Полный экран
exit-fullscreen = Выйти из полноэкранного режима
player-subtitles = Субтитры
player-audio = Аудио
subtitle-style-menu = Стиль и задержка
player-stats = Статистика потока
stats-download = Загрузка
stats-peers = Пиры
stats-downloaded = Загружено
stats-of = { $done } из { $total }
stats-network = Сеть
stats-buffered = В буфере
stats-resolution = Разрешение
stats-video = Видео
stats-decoder = Декодер
stats-software = Программный
stats-dropped = Пропущенные кадры
mute = Выключить звук
unmute = Включить звук
position = Позиция
volume = Громкость
subtitles-off = Выкл.
no-audio-tracks = Нет звуковых дорожек
track = Дорожка { $number }
delay = Задержка
delay-help = Только для этого видео.
subtitles-earlier = Субтитры на { $amount } раньше
subtitles-later = Субтитры на { $amount } позже
delay-reset = Задержка субтитров { $delay }, сбросить

section-interface = Интерфейс
section-player = Плеер
section-languages = Языки
section-subtitles = Субтитры
section-audio = Аудио
section-torrents = Торренты
section-data = Данные
section-keyboard = Клавиатура
section-about = О программе

ui-language = Язык
ui-language-help = Язык меню и сообщений Cineo.
ui-language-system = Системный ({ $language })
interface-size = Размер интерфейса
interface-size-help = Увеличивает или уменьшает текст, постеры и элементы управления.
start-page = Начальная страница
start-page-help = Открывается при запуске Cineo.

hardware-decoding = Аппаратное декодирование
hardware-decoding-help = Видео декодирует видеокарта, что экономит энергию. Отключите, если на видео появляются артефакты или чёрный экран.
seek-step = Шаг перемотки
seek-step-help = Насколько перематывают ←/→ и кнопки перемотки.
short-seek-step = Короткий шаг перемотки
short-seek-step-help = Насколько перематывает Shift+←/→.
hide-controls = Скрывать элементы управления через
hide-controls-help = Без движения мыши и нажатий клавиш во время воспроизведения.
escape-fullscreen = Esc сначала выходит из полноэкранного режима
escape-fullscreen-help = Если выключено, Esc сразу закрывает плеер.
pause-minimized = Пауза при сворачивании
remember-volume = Запоминать громкость
remember-volume-help = Каждое видео начинается с той громкостью, на которой закончилось предыдущее.
seconds =
    { $count ->
        [one] { $count } секунда
        [few] { $count } секунды
        [many] { $count } секунд
       *[other] { $count } секунды
    }

audio-language = Язык аудио
audio-language-help = Воспроизводится дорожка файла на этом языке, иначе дорожка по умолчанию.
audio-language-second = Второй язык аудио
audio-language-second-help = Используется, если в файле нет аудио на первом языке.
subtitle-language = Язык субтитров
subtitle-language-help = Включаются при запуске видео: из файла, если они там есть, иначе из аддона субтитров.
subtitle-language-second = Второй язык субтитров
subtitle-language-second-help = Используется, если нет субтитров на первом языке.
language-none = Нет

subtitle-size = Размер
subtitle-font = Шрифт
subtitle-bold = Жирный
subtitle-color = Цвет текста
subtitle-opacity = Непрозрачность текста
subtitle-outline = Обводка
subtitle-outline-help = Не рисуется, если есть фон.
subtitle-background = Фон
subtitle-raise = Отступ снизу
subtitle-raise-help = В процентах от высоты изображения.
subtitle-keep-styles = Сохранять оформление стилизованных субтитров
subtitle-keep-styles-help = Стилизованные (ASS) субтитры, частые в аниме, задают свои шрифты, цвета и положение. Если выключено, их заменяют настройки выше.
subtitle-preview = Так выглядят субтитры.
subtitle-preview-badge = ПРЕДПРОСМОТР
font-sans = Без засечек
font-serif = С засечками
font-mono = Моноширинный
color-white = Белый
color-yellow = Жёлтый
color-cyan = Голубой
color-green = Зелёный
outline-black = Чёрная
outline-gray = Серая
outline-none = Без обводки
background-none = Без фона
background-black = Чёрная подложка
background-gray = Серая подложка

audio-output = Аудиовыход
audio-output-help = Стерео сводит объёмный звук к двум динамикам или наушникам.
output-auto = Автоматически (объёмный)
output-stereo = Стерео
passthrough = Сквозной вывод
passthrough-help = Передаёт Dolby и DTS без декодирования на ресивер по HDMI или S/PDIF. Не включайте, если ваш ресивер их не декодирует, иначе звука может не быть.

p2p-enabled = Показывать и воспроизводить торрент-потоки
torrent-upload = Раздавать другим пирам
torrent-upload-help = Если выключено, Cineo только загружает. Некоторые пиры тогда отдают меньше, и торренты могут работать медленнее. Ваш IP-адрес всё равно виден пирам.
torrent-storage = Данные торрентов
torrent-storage-help = Cineo хранит только часть рядом с текущей позицией, не больше выбранного размера, в памяти или на диске. При перемотке назад за её пределы эта часть загружается заново. Кэш не сохраняется: данные удаляются, когда воспроизведение заканчивается. Применяется со следующего торрента.
storage-disk = На диске, { $size }
storage-memory = В памяти, { $size }
download-limit = Ограничение загрузки
upload-limit = Ограничение отдачи
next-torrent = Применяется со следующего торрента.
no-limit = Без ограничений
peer-limit = Пиров на торрент
peer-limit-help = Больше пиров может ускорить загрузку, но требует больше соединений.
dht = Искать пиров через DHT
dht-help = Находит пиров без трекеров. Если выключено, у многих торрентов будет меньше пиров.
private-network = Разрешить адреса локальной сети
private-network-help = Позволяет обращаться к аддонам, изображениям и пирам на 127.0.0.1 или в домашней сети, для аддонов на своём сервере. Выключенный вариант безопаснее. Вступает в силу после перезапуска Cineo.

binge-watching = Автоматически включать следующую серию
binge-watching-help = Когда серия заканчивается, следующая запускается из того же аддона или открывается список её потоков.
next-notice = Уведомление о следующей серии
next-notice-help = За сколько до конца предлагается следующая серия.
next-notice-off = Выкл.
next-episode = Следующая серия
next-episode-in = Следующая серия через { $seconds } с

watched-at = Считать просмотренным после
watched-at-help = Просмотренные видео начинаются сначала и пропадают из «Продолжить просмотр».
watched-percent = { $percent } % просмотрено
clear-history = Очистить историю просмотра
clear-history-help = Очищает библиотеку и «Продолжить просмотр».
clear-ellipsis = Очистить…
reset-settings = Сбросить все настройки
reset-settings-help = Все настройки на этой странице вернутся к значениям по умолчанию.
reset-ellipsis = Сбросить…
reset-confirm-title = Сбросить все настройки?
reset-confirm-text = Все настройки вернутся к значениям по умолчанию. Аддоны и библиотека сохранятся.
reset = Сбросить
clear-confirm-title = Очистить историю просмотра?
clear-confirm-text = Все элементы и их прогресс будут удалены из библиотеки. Это нельзя отменить.
clear = Очистить

key-or = { $a }  или  { $b }
key-space = Пробел
key-shift-wheel = Shift+колесо
key-mouse-back = кнопка мыши «назад»
shortcut-search = Поиск
shortcut-switch-page = Переключить страницу
shortcut-leave-detail = Покинуть страницу сведений
shortcut-scroll-row = Прокрутить ряд в сторону
shortcut-play-pause = Воспроизведение или пауза
shortcut-seek = Перемотка на шаг перемотки
shortcut-short-seek = Перемотка на короткий шаг
shortcut-volume = Громкость  ·  без звука
shortcut-fullscreen = Полный экран

about-license = Cineo — свободное ПО под лицензией MIT.
about-credits = Сделано с помощью Rust, egui, mpv и librqbit.

language-ara = Арабский
language-bul = Болгарский
language-cat = Каталанский
language-chi = Китайский
language-hrv = Хорватский
language-cze = Чешский
language-dan = Датский
language-dut = Нидерландский
language-eng = Английский
language-fin = Финский
language-fre = Французский
language-ger = Немецкий
language-gre = Греческий
language-heb = Иврит
language-hin = Хинди
language-hun = Венгерский
language-ind = Индонезийский
language-ita = Итальянский
language-jpn = Японский
language-kor = Корейский
language-nor = Норвежский
language-per = Персидский
language-pol = Польский
language-por = Португальский
language-rum = Румынский
language-rus = Русский
language-srp = Сербский
language-spa = Испанский
language-swe = Шведский
language-tha = Тайский
language-tur = Турецкий
language-ukr = Украинский
language-vie = Вьетнамский

notice-invalid-link = Cineo не может открыть эту ссылку
link-install-title = Установить этот аддон?
link-install-notice = Ссылка предлагает установить аддон ниже. Устанавливайте только аддоны, которым доверяете.
links-register = Открывать ссылки stremio:// и cineo://
links-register-help = Ссылки на аддоны и страницы открываются в Cineo. Stremio перестанет получать ссылки stremio://.
links-register-button = Использовать Cineo
links-registered = Готово
links-register-failed = Ошибка: { $detail }
configure = Настроить
problem-configuration-required = Этот аддон нужно настроить перед установкой. Настройте его, затем установите полученную ссылку.

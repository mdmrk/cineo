package io.github.mdmrk.cineo

import java.io.File
import java.io.InputStream

fun installSubtitleFont(font: InputStream, dir: File) {
    dir.mkdirs()
    dir.resolve("Inter-Regular.ttf").outputStream().use { font.copyTo(it) }
}

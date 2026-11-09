package io.github.mdmrk.cineo

import java.nio.file.Files
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test

class SubtitleFontTest {
    @Test
    fun theFontIsTheOnlyFileInTheFolderAndAnOlderCopyIsReplaced() {
        val dir = Files.createTempDirectory("fonts").toFile().resolve("nested")
        installSubtitleFont("a longer old font".byteInputStream(), dir)
        installSubtitleFont("font".byteInputStream(), dir)
        val files = dir.listFiles().orEmpty()
        assertEquals(1, files.size)
        assertArrayEquals("font".toByteArray(), files[0].readBytes())
    }
}

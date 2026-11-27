package io.github.mdmrk.cineo.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class SettingsSearchTest {
    private val groups = listOf(
        SettingsGroup(
            "player",
            "Reproductor",
            listOf(
                SettingEntry(listOf("Decodificación por hardware", "Usa la tarjeta gráfica"), "hardware"),
                SettingEntry(listOf("Paso de salto", "Cuánto saltan los botones"), "seek"),
            ),
        ),
        SettingsGroup(
            "subtitles",
            "Subtítulos",
            listOf(
                SettingEntry(emptyList(), "preview"),
                SettingEntry(listOf("Tamaño", "Se aplica a los subtítulos"), "size"),
            ),
        ),
    )

    private fun shown(query: String) = groups.search(query).map { group -> group.key to group.entries.map { it.content } }

    @Test
    fun everyWordMustAppearInARowIgnoringCaseAndAccents() {
        assertEquals(listOf("player" to listOf("hardware")), shown("  GRAFICA  decodificacion "))
        assertEquals(emptyList<Pair<String, List<String>>>(), shown("grafica salto"))
    }

    @Test
    fun aWordNamingAGroupKeepsTheWholeGroup() {
        assertEquals(listOf("subtitles" to listOf("preview", "size")), shown("subtitulos"))
    }

    @Test
    fun rowsWithoutTextOnlyShowWithoutASearch() {
        assertEquals(listOf("subtitles" to listOf("size")), shown("tamaño"))
        assertEquals(groups.map { it.key }, groups.search(" ").map { it.key })
    }
}

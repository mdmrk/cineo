package io.github.mdmrk.cineo.ui

import java.text.Normalizer
import java.util.Locale

internal class SettingEntry<T>(val texts: List<String>, val content: T, val divided: Boolean = true)

internal class SettingsGroup<T>(val key: String, val title: String, val entries: List<SettingEntry<T>>)

private val marks = Regex("\\p{Mn}+")

private fun fold(text: String) =
    marks.replace(Normalizer.normalize(text, Normalizer.Form.NFD), "").lowercase(Locale.ROOT)

private fun matches(words: List<String>, texts: List<String>): Boolean {
    val haystack = fold(texts.joinToString(" "))
    return words.all { it in haystack }
}

internal fun <T> List<SettingsGroup<T>>.search(query: String): List<SettingsGroup<T>> {
    val words = fold(query).split(Regex("\\s+")).filter { it.isNotEmpty() }
    if (words.isEmpty()) return this
    return mapNotNull { group ->
        if (matches(words, listOf(group.title))) return@mapNotNull group
        val entries = group.entries.filter { it.texts.isNotEmpty() && matches(words, it.texts) }
        if (entries.isEmpty()) null else SettingsGroup(group.key, group.title, entries)
    }
}

package io.github.mdmrk.cineo

import java.io.File
import javax.xml.parsers.DocumentBuilderFactory
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.w3c.dom.Element

class TranslationsTest {
    private val res = File("src/main/res")
    private val locales = listOf("es", "b+es+419", "pt", "ru", "it", "de", "fr", "zh")
    private val argument = Regex("""%(\d+\$[sd]|%)""")

    private fun read(folder: String): Map<String, List<String>> {
        val document = DocumentBuilderFactory.newInstance().newDocumentBuilder()
            .parse(File(res, "$folder/strings.xml"))
        val entries = mutableMapOf<String, List<String>>()
        val strings = document.getElementsByTagName("string")
        for (i in 0 until strings.length) {
            val element = strings.item(i) as Element
            if (element.getAttribute("translatable") == "false") continue
            entries[element.getAttribute("name")] = listOf(element.textContent)
        }
        val plurals = document.getElementsByTagName("plurals")
        for (i in 0 until plurals.length) {
            val element = plurals.item(i) as Element
            val items = element.getElementsByTagName("item")
            entries["plurals/" + element.getAttribute("name")] =
                (0 until items.length).map { items.item(it).textContent }
        }
        return entries
    }

    private fun arguments(text: String) = argument.findAll(text).map { it.value }.sorted().toList()

    @Test
    fun everyLocaleHasEveryStringWithTheSameArguments() {
        val english = read("values")
        for (locale in locales) {
            val translated = read("values-$locale")
            assertEquals("strings in values-$locale", english.keys, translated.keys)
            for ((name, texts) in translated) {
                val expected = arguments(english.getValue(name).last())
                for (text in texts) {
                    assertTrue("values-$locale $name is empty", text.isNotBlank())
                    assertEquals("arguments of values-$locale $name", expected, arguments(text))
                }
            }
        }
    }
}

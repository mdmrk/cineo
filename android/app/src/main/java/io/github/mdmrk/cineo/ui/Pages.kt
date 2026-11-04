package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.res.stringResource
import io.github.mdmrk.cineo.R

enum class Page(val label: Int) {
    HOME(R.string.page_home),
    LIBRARY(R.string.page_library),
    SETTINGS(R.string.page_settings),
}

@Composable
fun Pages(content: @Composable (Page, PaddingValues) -> Unit) {
    var page by rememberSaveable { mutableStateOf(Page.HOME) }
    Scaffold(
        bottomBar = {
            NavigationBar {
                Page.entries.forEach { entry ->
                    NavigationBarItem(
                        selected = entry == page,
                        onClick = { page = entry },
                        icon = {},
                        label = { Text(stringResource(entry.label)) },
                    )
                }
            }
        },
    ) { padding -> content(page, padding) }
}

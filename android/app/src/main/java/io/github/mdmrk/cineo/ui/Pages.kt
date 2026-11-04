package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.add
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.windowInsetsTopHeight
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import io.github.mdmrk.cineo.R

enum class Page(val label: Int, val icon: Icon) {
    HOME(R.string.page_home, Icon.HOME),
    SEARCH(R.string.page_search, Icon.SEARCH),
    LIBRARY(R.string.page_library, Icon.BOOKS),
    ADDONS(R.string.page_addons, Icon.PUZZLE),
    SETTINGS(R.string.page_settings, Icon.SETTINGS),
}

@Composable
fun Pages(page: Page, onPage: (Page) -> Unit, content: @Composable (PaddingValues) -> Unit) {
    Scaffold(
        containerColor = Brand.Background,
        bottomBar = {
            NavigationBar(containerColor = Brand.Panel) {
                Page.entries.forEach { entry ->
                    val selected = entry == page
                    NavigationBarItem(
                        selected = selected,
                        onClick = { onPage(entry) },
                        icon = { CineoIcon(entry.icon, filled = selected) },
                        label = { Text(stringResource(entry.label), style = MaterialTheme.typography.labelSmall) },
                        colors = NavigationBarItemDefaults.colors(
                            selectedIconColor = Brand.Accent,
                            selectedTextColor = Brand.TextBright,
                            unselectedIconColor = Brand.TextDim,
                            unselectedTextColor = Brand.TextDim,
                            indicatorColor = Brand.Surface,
                        ),
                    )
                }
            }
        },
    ) { padding ->
        Box(Modifier.fillMaxSize()) {
            content(padding)
            Box(
                Modifier.fillMaxWidth()
                    .windowInsetsTopHeight(WindowInsets.statusBars.add(WindowInsets(top = 20.dp)))
                    .background(
                        Brush.verticalGradient(
                            0f to Brand.Background,
                            0.6f to Brand.Background.copy(alpha = 0.85f),
                            1f to Color.Transparent,
                        ),
                    ),
            )
        }
    }
}

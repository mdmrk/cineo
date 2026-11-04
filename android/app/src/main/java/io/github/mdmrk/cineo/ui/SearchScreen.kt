package io.github.mdmrk.cineo.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import io.github.mdmrk.cineo.R
import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.PosterView
import kotlinx.coroutines.delay

private const val SEARCH_DELAY_MS = 500L

@Composable
fun SearchScreen(
    view: AppView?,
    images: Images,
    onSearch: (String) -> Unit,
    onOpen: (PosterView) -> Unit,
    padding: PaddingValues,
) {
    var query by rememberSaveable { mutableStateOf(view?.searchQuery.orEmpty()) }
    val focus = LocalFocusManager.current
    LaunchedEffect(query) {
        if (query.trim() != view?.searchQuery) {
            delay(SEARCH_DELAY_MS)
            onSearch(query)
        }
    }
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        contentPadding = PaddingValues(
            top = padding.calculateTopPadding() + 16.dp,
            bottom = padding.calculateBottomPadding() + 16.dp,
        ),
        verticalArrangement = Arrangement.spacedBy(24.dp),
    ) {
        item(key = "field") {
            OutlinedTextField(
                value = query,
                onValueChange = { query = it },
                modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
                singleLine = true,
                placeholder = { Text(stringResource(R.string.search_hint)) },
                leadingIcon = { CineoIcon(Icon.SEARCH, tint = Brand.TextDim) },
                trailingIcon = {
                    if (query.isNotEmpty()) {
                        IconButton(onClick = { query = "" }) { CineoIcon(Icon.X, tint = Brand.TextDim) }
                    }
                },
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = {
                    onSearch(query)
                    focus.clearFocus()
                }),
            )
        }
        val rows = view?.search.orEmpty()
        when {
            view?.searchQuery.isNullOrEmpty() -> item(key = "help") {
                Text(stringResource(R.string.search_help), color = Brand.TextDim, modifier = Modifier.padding(horizontal = 16.dp))
            }
            rows.isEmpty() -> item(key = "unsupported") {
                Text(stringResource(R.string.search_unsupported), color = Brand.TextDim, modifier = Modifier.padding(horizontal = 16.dp))
            }
            else -> items(rows, key = { it.key }) { row -> CatalogRow(row, images, onOpen) }
        }
    }
}

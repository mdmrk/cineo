package io.github.mdmrk.cineo

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.platform.LocalContext
import androidx.core.content.ContextCompat
import io.github.mdmrk.cineo.ui.ConnectingScreen
import io.github.mdmrk.cineo.ui.P2pPrompt
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.mdmrk.cineo.ui.AddonsScreen
import io.github.mdmrk.cineo.ui.CineoTheme
import io.github.mdmrk.cineo.ui.SearchScreen
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import io.github.mdmrk.cineo.ui.DetailScreen
import io.github.mdmrk.cineo.ui.HomeScreen
import io.github.mdmrk.cineo.ui.Images
import io.github.mdmrk.cineo.ui.LibraryActions
import io.github.mdmrk.cineo.ui.LibraryScreen
import io.github.mdmrk.cineo.ui.Page
import io.github.mdmrk.cineo.ui.Pages
import io.github.mdmrk.cineo.ui.Notices
import io.github.mdmrk.cineo.ui.PlayerScreen
import io.github.mdmrk.cineo.ui.SettingsScreen
import io.github.mdmrk.cineo.ui.StartFailed

@Composable
private fun TorrentForeground(active: Boolean) {
    val context = LocalContext.current
    val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {}
    LaunchedEffect(active) {
        if (active) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) !=
                PackageManager.PERMISSION_GRANTED
            ) {
                ask.launch(Manifest.permission.POST_NOTIFICATIONS)
            }
            TorrentService.start(context)
        } else {
            TorrentService.stop(context)
        }
    }
}

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        val app = application as CineoApplication
        setContent {
            CineoTheme {
                when (val started = app.started) {
                    is Started.Running -> {
                        val cineo = started.cineo
                        val view by app.view.collectAsStateWithLifecycle()
                        val images = remember(cineo) { Images(cineo) }
                        var page by rememberSaveable { mutableStateOf(Page.HOME) }
                        val current = view
                        val playback = current?.playback
                        val detail = current?.detail
                        val connecting = current?.connecting
                        TorrentForeground(current?.torrent != null)
                        when {
                            playback != null -> PlayerScreen(playback, current.torrent, cineo)
                            connecting != null -> ConnectingScreen(connecting, current.torrent, cineo::stopPlayback)
                            detail != null -> {
                                BackHandler { cineo.closeDetail() }
                                DetailScreen(
                                    detail = detail,
                                    resume = current.continueWatching.firstOrNull { it.id == detail.id },
                                    images = images,
                                    onBack = cineo::closeDetail,
                                    onResume = { cineo.resume(detail.id) },
                                    onSelectVideo = cineo::selectVideo,
                                    onPlay = cineo::play,
                                    onFavorite = { cineo.setFavorite(detail.id, it) },
                                )
                            }
                            else -> {
                                val library = remember(cineo) {
                                    LibraryActions(
                                        open = { cineo.openDetail(it.contentType, it.id) },
                                        resume = { cineo.resume(it.id) },
                                        setFavorite = { item, favorite -> cineo.setFavorite(item.id, favorite) },
                                        remove = { cineo.removeFromLibrary(it.id) },
                                    )
                                }
                                BackHandler(enabled = page != Page.HOME) { page = Page.HOME }
                                Pages(page, { page = it }) { padding ->
                                    val open = { poster: io.github.mdmrk.cineo.rust.PosterView ->
                                        cineo.openDetail(poster.contentType, poster.id)
                                    }
                                    when (page) {
                                        Page.HOME -> HomeScreen(
                                            view = current,
                                            images = images,
                                            onOpen = open,
                                            onOpenAddons = { page = Page.ADDONS },
                                            library = library,
                                            padding = padding,
                                        )
                                        Page.SEARCH -> SearchScreen(current, images, cineo::search, open, padding)
                                        Page.LIBRARY -> LibraryScreen(current, images, library, padding)
                                        Page.ADDONS -> AddonsScreen(current, cineo, padding)
                                        Page.SETTINGS -> current?.let { SettingsScreen(it.settings, cineo, padding) }
                                    }
                                }
                            }
                        }
                        if (current?.p2pPrompt == true) {
                            P2pPrompt(onAccept = cineo::acceptP2p, onDecline = cineo::declineP2p)
                        }
                        Notices(current, aboveTabs = playback == null && connecting == null && detail == null, onDismissNotice = cineo::dismissNotice)
                    }
                    is Started.Failed -> StartFailed(started.reason)
                }
            }
        }
    }
}

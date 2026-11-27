package io.github.mdmrk.cineo

import android.Manifest
import android.app.PictureInPictureParams
import android.content.Intent
import android.content.res.Configuration
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
import io.github.mdmrk.cineo.rust.PosterView
import io.github.mdmrk.cineo.rust.RemoteLinkView
import io.github.mdmrk.cineo.rust.SyncLinkView
import io.github.mdmrk.cineo.rust.remoteLink
import io.github.mdmrk.cineo.rust.syncLink
import io.github.mdmrk.cineo.ui.RemoteBar
import io.github.mdmrk.cineo.ui.RemoteScreen
import io.github.mdmrk.cineo.ui.localNetworkAllowed
import io.github.mdmrk.cineo.ui.remoteFailureText
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.RemoteFailure
import android.widget.Toast
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.launch
import io.github.mdmrk.cineo.ui.AddonsScreen
import io.github.mdmrk.cineo.ui.CineoTheme
import io.github.mdmrk.cineo.ui.SearchScreen
import io.github.mdmrk.cineo.ui.StreamFilterActions
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.setValue
import io.github.mdmrk.cineo.ui.DetailScreen
import io.github.mdmrk.cineo.ui.HomeScreen
import io.github.mdmrk.cineo.ui.Images
import io.github.mdmrk.cineo.ui.LibraryActions
import io.github.mdmrk.cineo.ui.LibraryScreen
import io.github.mdmrk.cineo.ui.Page
import io.github.mdmrk.cineo.ui.Pages
import io.github.mdmrk.cineo.ui.Notices
import io.github.mdmrk.cineo.ui.Pip
import io.github.mdmrk.cineo.ui.PlayerScreen
import io.github.mdmrk.cineo.ui.canPictureInPicture
import io.github.mdmrk.cineo.ui.pictureInPictureParams
import androidx.lifecycle.Lifecycle
import io.github.mdmrk.cineo.ui.SettingsScreen
import io.github.mdmrk.cineo.ui.StartFailed

@Composable
private fun PlaybackForeground(active: Boolean) {
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
            PlaybackService.start(context)
        }
    }
}

class MainActivity : ComponentActivity() {
    private val network by lazy {
        NetworkWatch(this) { renew -> cineo?.networkUsable(renew) }
    }

    private val cineo get() = (application as CineoApplication).cineo

    private var inPip by mutableStateOf(false)
    private var pendingSync by mutableStateOf<SyncLinkView?>(null)
    private var pendingRemote by mutableStateOf<RemoteLinkView?>(null)

    private fun linkOf(intent: Intent?): String? =
        intent?.takeIf { it.action == Intent.ACTION_VIEW }?.dataString

    private fun takeLinks(intent: Intent?) {
        val link = linkOf(intent) ?: return
        syncLink(link)?.let { pendingSync = it }
        remoteLink(link)?.let { pendingRemote = it }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        takeLinks(intent)
    }
    private var pipOnLeave: PictureInPictureParams? = null

    private fun playOnComputer(cineo: Cineo, group: UInt, stream: UInt) {
        if (!localNetworkAllowed(this)) {
            Toast.makeText(this, R.string.remote_local_network, Toast.LENGTH_LONG).show()
            return
        }
        lifecycleScope.launch {
            val message = try {
                cineo.playOnComputer(group, stream)
                getString(R.string.remote_sent)
            } catch (e: RemoteFailure) {
                remoteFailureText(this@MainActivity, e)
            }
            Toast.makeText(this@MainActivity, message, Toast.LENGTH_LONG).show()
        }
    }

    override fun onStart() {
        super.onStart()
        network.start()
        cineo?.appVisible(true)
    }

    override fun onStop() {
        network.stop()
        if (!isChangingConfigurations) cineo?.appVisible(false)
        super.onStop()
    }

    override fun onUserLeaveHint() {
        super.onUserLeaveHint()
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) pipOnLeave?.let(::enterPictureInPictureMode)
    }

    override fun onPictureInPictureModeChanged(isInPictureInPictureMode: Boolean, newConfig: Configuration) {
        super.onPictureInPictureModeChanged(isInPictureInPictureMode, newConfig)
        inPip = isInPictureInPictureMode
        if (!inPip && !lifecycle.currentState.isAtLeast(Lifecycle.State.STARTED)) {
            (application as CineoApplication).player?.pause()
        }
    }

    private fun pictureInPicture(pip: Pip) {
        if (!canPictureInPicture()) return
        val params = pictureInPictureParams(pip)
        setPictureInPictureParams(params)
        pipOnLeave = params.takeIf { pip.auto }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge(
            statusBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
            navigationBarStyle = SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),
        )
        super.onCreate(savedInstanceState)
        val app = application as CineoApplication
        takeLinks(intent)
        setContent {
            CineoTheme {
                when (val started = app.started) {
                    is Started.Running -> {
                        val cineo = started.cineo
                        val view by app.view.collectAsStateWithLifecycle()
                        val player = app.player ?: return@CineoTheme
                        val images = remember(cineo) { Images(cineo) }
                        var page by rememberSaveable { mutableStateOf(Page.HOME) }
                        var remoteOpen by rememberSaveable { mutableStateOf(false) }
                        val savedPages = rememberSaveableStateHolder()
                        val current = view
                        val playback = current?.playback
                        val detail = current?.detail
                        LaunchedEffect(pendingSync, pendingRemote) {
                            if (pendingSync != null || pendingRemote != null) {
                                page = Page.SETTINGS
                                remoteOpen = false
                                cineo.closeDetail()
                            }
                        }
                        val remote = current?.remote
                        val connecting = current?.connecting
                        PlaybackForeground(playback != null || connecting != null || current?.torrent != null)
                        LaunchedEffect(inPip, playback == null && connecting == null) {
                            if (inPip && playback == null && connecting == null) moveTaskToBack(true)
                        }
                        UpdateNotification(current?.update, canAsk = playback == null && connecting == null)
                        when {
                            playback != null -> PlayerScreen(
                                playback = playback,
                                torrent = current.torrent,
                                images = images,
                                cineo = cineo,
                                player = player,
                                inPip = inPip,
                                pipOnLeave = current.settings.onLeave.value == "pip",
                                onPip = ::pictureInPicture,
                            )
                            connecting != null -> ConnectingScreen(connecting, current.torrent, images, cineo::stopPlayback)
                            remoteOpen && remote != null -> {
                                BackHandler { remoteOpen = false }
                                RemoteScreen(remote, cineo) { remoteOpen = false }
                            }
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
                                    onPlayOnComputer = remote?.let {
                                        { group, stream -> playOnComputer(cineo, group, stream) }
                                    },
                                    onFavorite = { cineo.setFavorite(detail.id, it) },
                                    filters = remember(cineo) {
                                        StreamFilterActions(
                                            query = cineo::setStreamQuery,
                                            quality = cineo::setStreamQuality,
                                            addon = cineo::setStreamAddon,
                                            sort = cineo::setStreamSort,
                                        )
                                    },
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
                                Pages(page, { page = it }, bar = { RemoteBar(remote, cineo) { remoteOpen = true } }) { padding ->
                                    val open = { poster: PosterView ->
                                        cineo.openDetail(poster.contentType, poster.id)
                                    }
                                    savedPages.SaveableStateProvider(page.name) {
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
                                            Page.ADDONS -> AddonsScreen(current, images, cineo, padding)
                                            Page.SETTINGS -> current?.let {
                                                SettingsScreen(
                                                    it.settings,
                                                    it.update,
                                                    it.sync,
                                                    pendingSync,
                                                    { pendingSync = null },
                                                    it.remote,
                                                    pendingRemote,
                                                    { pendingRemote = null },
                                                    { remoteOpen = true },
                                                    cineo,
                                                    padding,
                                                )
                                            }
                                        }
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

package io.github.mdmrk.cineo

import io.github.mdmrk.cineo.rust.AppView
import io.github.mdmrk.cineo.rust.Cineo
import io.github.mdmrk.cineo.rust.PlayerStatusView
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class Player(private val cineo: Cineo, views: StateFlow<AppView?>, scope: CoroutineScope) {
    private val statuses = MutableStateFlow<PlayerStatusView?>(null)
    val status: StateFlow<PlayerStatusView?> = statuses

    init {
        scope.launch {
            views.map { it?.playback != null }.distinctUntilChanged().collectLatest { playing ->
                statuses.value = null
                while (playing) {
                    statuses.value = withContext(Dispatchers.Default) { cineo.playerStatus() }
                    delay(POLL_MS)
                }
            }
        }
    }

    fun play() {
        statuses.update { it?.copy(paused = false) }
        cineo.resumePlayback()
    }

    fun pause() {
        statuses.update { it?.copy(paused = true) }
        cineo.pause()
    }

    fun togglePause() {
        if (statuses.value?.paused == true) play() else pause()
    }

    fun seekTo(seconds: Double) = cineo.seekTo(seconds)

    fun seekBy(seconds: Double) = cineo.seekBy(seconds)

    fun speed(speed: Double) = cineo.setSpeed(speed)

    fun next() = cineo.playNext()

    fun stop() = cineo.stopPlayback()

    private companion object {
        const val POLL_MS = 250L
    }
}

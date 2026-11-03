package io.github.mdmrk.cineo

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.mdmrk.cineo.ui.CineoTheme
import io.github.mdmrk.cineo.ui.HomeScreen
import io.github.mdmrk.cineo.ui.Images
import io.github.mdmrk.cineo.ui.StartFailed

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)
        val app = application as CineoApplication
        setContent {
            CineoTheme {
                when (val started = app.started) {
                    is Started.Running -> {
                        val cineo = started.cineo
                        val view by app.view.collectAsStateWithLifecycle()
                        val images = remember(cineo) { Images(cineo) }
                        HomeScreen(
                            view = view,
                            images = images,
                            onInstall = cineo::installAddon,
                            onDismissNotice = cineo::dismissNotice,
                        )
                    }
                    is Started.Failed -> StartFailed(started.reason)
                }
            }
        }
    }
}

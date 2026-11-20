package com.pcmonitor.app.update

import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp

/** "Update available" dialog, shown over any screen while AppUpdater has something to say. */
@Composable
fun UpdateDialog() {
    val state by AppUpdater.state.collectAsState()
    val context = LocalContext.current

    when (val s = state) {
        UpdateState.Idle -> Unit
        is UpdateState.Available -> AlertDialog(
            onDismissRequest = AppUpdater::dismiss,
            title = { Text("Update available") },
            text = { Text("Vitals ${s.release.version} is ready.\nYou have ${s.installedVersion}.") },
            dismissButton = { TextButton(onClick = AppUpdater::dismiss) { Text("Later") } },
            confirmButton = {
                TextButton(onClick = { AppUpdater.startUpdate(context, s.release) }) {
                    Text("Update")
                }
            },
        )
        is UpdateState.Downloading -> AlertDialog(
            onDismissRequest = {},
            title = { Text("Downloading Vitals ${s.release.version}") },
            text = {
                Column {
                    Spacer(Modifier.height(8.dp))
                    LinearProgressIndicator(progress = { s.progress }, modifier = Modifier.fillMaxWidth())
                }
            },
            confirmButton = {},
        )
        is UpdateState.Installing -> AlertDialog(
            onDismissRequest = {},
            title = { Text("Installing Vitals ${s.release.version}") },
            text = { Text("Confirm the update on the next screen. Vitals restarts when it's done.") },
            confirmButton = {},
        )
        is UpdateState.Failed -> AlertDialog(
            onDismissRequest = AppUpdater::dismiss,
            title = { Text("Update didn't finish") },
            text = { Text("${s.message}\n\nYou can tap Try Again or download directly in your mobile browser.") },
            dismissButton = {
                Row {
                    TextButton(onClick = AppUpdater::dismiss) { Text("Later") }
                    TextButton(onClick = {
                        val browserIntent = Intent(Intent.ACTION_VIEW, Uri.parse(s.release.apkUrl))
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        context.startActivity(browserIntent)
                        AppUpdater.dismiss()
                    }) {
                        Text("Open in Browser")
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { AppUpdater.startUpdate(context, s.release) }) {
                    Text("Try again")
                }
            },
        )
    }
}

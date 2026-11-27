package com.pcmonitor.app.update

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.pcmonitor.app.ui.components.AppLogo
import com.pcmonitor.app.ui.components.RetroButton
import com.pcmonitor.app.ui.components.RetroButtonVariant
import com.pcmonitor.app.ui.components.RetroProgressBar
import com.pcmonitor.app.ui.theme.ArcadeYellow
import com.pcmonitor.app.ui.theme.CreamBackground
import com.pcmonitor.app.ui.theme.CreamBorder
import com.pcmonitor.app.ui.theme.CreamSurface
import com.pcmonitor.app.ui.theme.CreamText
import com.pcmonitor.app.ui.theme.CreamTextSecondary
import com.pcmonitor.app.ui.theme.CriticalRed
import com.pcmonitor.app.ui.theme.DarkNavyLight
import com.pcmonitor.app.ui.theme.DarkNavySurface
import com.pcmonitor.app.ui.theme.PixelBorderLight
import com.pcmonitor.app.ui.theme.PixelFontFamily
import com.pcmonitor.app.ui.theme.PixelGreen
import com.pcmonitor.app.ui.theme.ReadableFontFamily
import com.pcmonitor.app.ui.theme.TextGray
import com.pcmonitor.app.ui.theme.TextWhite

/**
 * "Update available" pop-up, shown over any screen while AppUpdater has something to say.
 * Drawn in the app's own retro style (navy panel, pixel title, beige info card, RetroButtons)
 * instead of the stock Material dialog, which looked like a different app.
 */
@Composable
fun UpdateDialog() {
    val state by AppUpdater.state.collectAsState()
    val context = LocalContext.current

    when (val s = state) {
        UpdateState.Idle -> Unit
        is UpdateState.Available -> RetroUpdatePanel(
            title = "UPDATE READY",
            accent = ArcadeYellow,
            onDismiss = AppUpdater::dismiss,
        ) {
            VersionRow(from = s.installedVersion, to = s.release.version)
            Spacer(Modifier.height(12.dp))
            Body("A new version of Vitals is ready to install. Your devices and settings stay as they are.")
            Spacer(Modifier.height(18.dp))
            RetroButton(
                text = "UPDATE NOW",
                onClick = { AppUpdater.startUpdate(context, s.release) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(10.dp))
            RetroButton(
                text = "LATER",
                onClick = AppUpdater::dismiss,
                modifier = Modifier.fillMaxWidth(),
                variant = RetroButtonVariant.Outline,
            )
        }
        is UpdateState.Downloading -> RetroUpdatePanel(
            title = "DOWNLOADING",
            accent = PixelGreen,
            onDismiss = null,
        ) {
            Body("Vitals ${s.release.version}")
            Spacer(Modifier.height(14.dp))
            RetroProgressBar(
                progress = s.progress.coerceIn(0f, 1f),
                modifier = Modifier.fillMaxWidth(),
                color = PixelGreen,
                trackColor = DarkNavyLight,
                height = 14.dp,
            )
            Spacer(Modifier.height(8.dp))
            Text(
                text = "${(s.progress.coerceIn(0f, 1f) * 100).toInt()}%",
                fontFamily = PixelFontFamily,
                fontSize = 10.sp,
                color = TextGray,
                modifier = Modifier.fillMaxWidth(),
                textAlign = TextAlign.End,
            )
        }
        is UpdateState.Installing -> RetroUpdatePanel(
            title = "INSTALLING",
            accent = PixelGreen,
            onDismiss = null,
        ) {
            Body("Confirm the update on the next screen. Vitals ${s.release.version} restarts when it's done.")
        }
        is UpdateState.Failed -> RetroUpdatePanel(
            title = "UPDATE FAILED",
            accent = CriticalRed,
            onDismiss = AppUpdater::dismiss,
        ) {
            Body(s.message)
            Spacer(Modifier.height(18.dp))
            RetroButton(
                text = "TRY AGAIN",
                onClick = { AppUpdater.startUpdate(context, s.release) },
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(10.dp))
            RetroButton(
                text = "LATER",
                onClick = AppUpdater::dismiss,
                modifier = Modifier.fillMaxWidth(),
                variant = RetroButtonVariant.Outline,
            )
        }
    }
}

/** Navy panel with a 2 dp pixel border, the logo and a pixel-font title (DESIGN.md: zero blur). */
@Composable
private fun RetroUpdatePanel(
    title: String,
    accent: Color,
    onDismiss: (() -> Unit)?,
    content: @Composable () -> Unit,
) {
    Dialog(
        onDismissRequest = { onDismiss?.invoke() },
        properties = DialogProperties(
            dismissOnBackPress = onDismiss != null,
            dismissOnClickOutside = onDismiss != null,
        ),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .background(DarkNavySurface, RoundedCornerShape(8.dp))
                .border(2.dp, PixelBorderLight, RoundedCornerShape(8.dp))
                .padding(20.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            AppLogo(size = 56.dp)
            Spacer(Modifier.height(12.dp))
            Text(
                text = title,
                fontFamily = PixelFontFamily,
                fontSize = 14.sp,
                fontWeight = FontWeight.Bold,
                color = accent,
                textAlign = TextAlign.Center,
            )
            Spacer(Modifier.height(6.dp))
            // Thin accent rule under the title, like the app's section headers.
            Box(
                Modifier
                    .width(48.dp)
                    .height(3.dp)
                    .background(accent),
            )
            Spacer(Modifier.height(16.dp))
            content()
        }
    }
}

/** Beige card with "installed → new" version chips. */
@Composable
private fun VersionRow(from: String, to: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(CreamBackground, RoundedCornerShape(6.dp))
            .border(1.5.dp, CreamBorder, RoundedCornerShape(6.dp))
            .padding(horizontal = 12.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween,
    ) {
        VersionChip(label = "YOU HAVE", version = from, highlight = false)
        Text(text = "→", fontFamily = PixelFontFamily, fontSize = 14.sp, color = CreamText)
        VersionChip(label = "NEW", version = to, highlight = true)
    }
}

@Composable
private fun VersionChip(label: String, version: String, highlight: Boolean) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(text = label, fontFamily = PixelFontFamily, fontSize = 8.sp, color = CreamTextSecondary)
        Spacer(Modifier.height(6.dp))
        Box(
            modifier = Modifier
                .background(if (highlight) PixelGreen else CreamSurface, RoundedCornerShape(4.dp))
                .border(1.dp, CreamBorder, RoundedCornerShape(4.dp))
                .padding(horizontal = 10.dp, vertical = 6.dp),
        ) {
            Text(
                text = "v$version",
                fontFamily = PixelFontFamily,
                fontSize = 11.sp,
                color = if (highlight) TextWhite else CreamText,
            )
        }
    }
}

@Composable
private fun Body(text: String) {
    Text(
        text = text,
        fontFamily = ReadableFontFamily,
        fontSize = 14.sp,
        color = TextGray,
        textAlign = TextAlign.Center,
        modifier = Modifier.fillMaxWidth(),
    )
}

package com.pcmonitor.app.ui.screens.pairing

import android.content.Intent
import android.os.Build
import android.widget.Toast
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Share
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Pin
import androidx.compose.material.icons.filled.QrCodeScanner
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.pcmonitor.app.ui.components.PixelCharacter
import com.pcmonitor.app.ui.components.PixelScanScene
import com.pcmonitor.app.ui.components.PixelComputer
import com.pcmonitor.app.ui.components.RetroTopBar
import com.pcmonitor.app.ui.theme.ArcadeYellow
import com.pcmonitor.app.ui.theme.CardBackground
import com.pcmonitor.app.ui.theme.DeepNavy
import com.pcmonitor.app.ui.theme.PixelBorderColor
import com.pcmonitor.app.ui.theme.PixelFontFamily
import com.pcmonitor.app.ui.theme.PixelGreen
import com.pcmonitor.app.ui.theme.ReadableFontFamily
import com.pcmonitor.app.ui.theme.TextDarkGray
import com.pcmonitor.app.ui.theme.TextGray
import com.pcmonitor.app.ui.theme.TextWhite

@Composable
fun AddDeviceScreen(
    onBack: () -> Unit,
    onScanQrClick: () -> Unit,
    onEnterCodeClick: () -> Unit
) {
    val isDark = MaterialTheme.colorScheme.background.red < 0.5f

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
    ) {
        RetroTopBar(
            title = "Add New Device",
            onBackClick = onBack
        )

        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            // Cream-colored card with pixel art illustration
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(150.dp)
                    .clip(RoundedCornerShape(12.dp))
                    .background(Color(0xFFF9F6EE))
                    .border(2.dp, Color(0xFF8B7355), RoundedCornerShape(12.dp)),
                contentAlignment = Alignment.Center
            ) {
                PixelScanScene(modifier = Modifier.size(width = 190.dp, height = 120.dp))
            }

            Spacer(modifier = Modifier.height(24.dp))

            StepHeading(number = 1, title = "Install on the computer")
            Spacer(modifier = Modifier.height(10.dp))
            InstallCommandCard()

            Spacer(modifier = Modifier.height(28.dp))

            StepHeading(number = 2, title = "Pair it")
            Spacer(modifier = Modifier.height(10.dp))

            // Option 1: Scan QR Code
            PairingOptionCard(
                icon = Icons.Default.QrCodeScanner,
                title = "Scan QR Code",
                description = "Scan the QR code displayed on your PC screen.",
                onClick = onScanQrClick
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Option 2: Enter Pairing Code
            PairingOptionCard(
                icon = Icons.Default.Pin,
                title = "Enter Pairing Code",
                description = "Type the 6-character code from your PC screen.",
                onClick = onEnterCodeClick
            )
        }
    }
}

/** The one-line installers published with every GitHub release (installer/get.ps1, installer/linux/get.sh). */
private enum class InstallTarget(val label: String, val where: String, val command: String) {
    Windows(
        "Windows",
        "Paste into PowerShell on the computer:",
        "irm https://github.com/theallmyti/Vitals/releases/latest/download/install.ps1 | iex"
    ),
    Linux(
        "Linux",
        "Paste into a terminal on the computer:",
        "curl -fsSL https://github.com/theallmyti/Vitals/releases/latest/download/install.sh | sh"
    )
}

@Composable
private fun StepHeading(number: Int, title: String) {
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Box(
            modifier = Modifier
                .size(24.dp)
                .clip(RoundedCornerShape(6.dp))
                .background(Color(0xFF0F1B2E))
                .border(1.5.dp, PixelGreen, RoundedCornerShape(6.dp)),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = number.toString(),
                fontFamily = PixelFontFamily,
                fontWeight = FontWeight.Bold,
                fontSize = 10.sp,
                color = PixelGreen
            )
        }
        Spacer(modifier = Modifier.width(10.dp))
        Text(
            text = title,
            fontFamily = PixelFontFamily,
            fontWeight = FontWeight.Bold,
            fontSize = 12.sp,
            color = MaterialTheme.colorScheme.onBackground
        )
    }
}

@Composable
private fun InstallCommandCard() {
    val context = LocalContext.current
    val clipboard = LocalClipboardManager.current
    var targetName by rememberSaveable { mutableStateOf(InstallTarget.Windows.name) }
    val target = InstallTarget.valueOf(targetName)

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(Color(0xFFFDF6E3))
            .border(1.5.dp, Color(0xFF8B7355), RoundedCornerShape(10.dp))
            .padding(14.dp)
    ) {
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            InstallTarget.values().forEach { option ->
                val selected = option == target
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .background(if (selected) Color(0xFF0F1B2E) else Color.Transparent)
                        .border(1.5.dp, if (selected) PixelGreen else Color(0xFF8B7355), RoundedCornerShape(6.dp))
                        .clickable { targetName = option.name }
                        .padding(horizontal = 14.dp, vertical = 7.dp)
                ) {
                    Text(
                        text = option.label,
                        fontFamily = PixelFontFamily,
                        fontSize = 10.sp,
                        color = if (selected) PixelGreen else Color(0xFF0F172A)
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(12.dp))
        Text(
            text = target.where,
            fontFamily = ReadableFontFamily,
            fontSize = 12.sp,
            color = Color(0xFF64748B)
        )
        Spacer(modifier = Modifier.height(6.dp))

        SelectionContainer {
            Text(
                text = target.command,
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(6.dp))
                    .background(Color(0xFF0F1B2E))
                    .padding(10.dp),
                fontFamily = FontFamily.Monospace,
                fontSize = 12.sp,
                lineHeight = 17.sp,
                color = PixelGreen
            )
        }

        Spacer(modifier = Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            CommandButton(icon = Icons.Default.ContentCopy, label = "Copy") {
                clipboard.setText(AnnotatedString(target.command))
                // Android 13+ shows its own "copied" confirmation; older versions show nothing.
                if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
                    Toast.makeText(context, "Copied", Toast.LENGTH_SHORT).show()
                }
            }
            CommandButton(icon = Icons.Default.Share, label = "Share") {
                // Send it to yourself (chat, email, notes) and open it on the computer.
                val send = Intent(Intent.ACTION_SEND).apply {
                    type = "text/plain"
                    putExtra(Intent.EXTRA_SUBJECT, "Install Vitals on ${target.label}")
                    putExtra(Intent.EXTRA_TEXT, target.command)
                }
                context.startActivity(Intent.createChooser(send, "Send the install command"))
            }
        }
    }
}

@Composable
private fun CommandButton(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    label: String,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier
            .clip(RoundedCornerShape(6.dp))
            .border(1.5.dp, Color(0xFF8B7355), RoundedCornerShape(6.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 12.dp, vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(imageVector = icon, contentDescription = null, tint = Color(0xFF0F172A), modifier = Modifier.size(16.dp))
        Spacer(modifier = Modifier.width(6.dp))
        Text(text = label, fontFamily = PixelFontFamily, fontSize = 10.sp, color = Color(0xFF0F172A))
    }
}

@Composable
private fun PairingOptionCard(
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    title: String,
    description: String,
    onClick: () -> Unit
) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(10.dp))
            .background(Color(0xFFFDF6E3))
            .border(1.5.dp, Color(0xFF8B7355), RoundedCornerShape(10.dp))
            .clickable(onClick = onClick)
            .padding(16.dp)
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(46.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(Color(0xFF0F1B2E))
                    .border(1.5.dp, PixelGreen, RoundedCornerShape(8.dp)),
                contentAlignment = Alignment.Center
            ) {
                Icon(
                    imageVector = icon,
                    contentDescription = title,
                    tint = PixelGreen,
                    modifier = Modifier.size(26.dp)
                )
            }

            Spacer(modifier = Modifier.width(14.dp))

            Column {
                Text(
                    text = title,
                    fontFamily = PixelFontFamily,
                    fontWeight = FontWeight.Bold,
                    fontSize = 11.sp,
                    lineHeight = 17.sp,
                    color = Color(0xFF0F172A)
                )
                Spacer(modifier = Modifier.height(3.dp))
                Text(
                    text = description,
                    fontFamily = ReadableFontFamily,
                    fontSize = 12.sp,
                    color = Color(0xFF64748B)
                )
            }
        }
    }
}


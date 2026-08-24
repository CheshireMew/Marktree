package io.github.cheshiremew.marktree.bridge

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.io.File
import java.util.ArrayDeque
import java.util.UUID
import java.util.concurrent.Executors

data class IncomingShare(
    val text: String?,
    val subject: String?,
    val filePath: String?,
    val fileName: String?,
    val mediaType: String?,
)

@InvokeArg
class ShareFileArgs {
    lateinit var path: String
    lateinit var mediaType: String
    lateinit var title: String
}

@TauriPlugin
class SharePlugin(private val activity: Activity) : Plugin(activity) {
    companion object {
        private const val MAX_INCOMING_SHARE_BYTES = 256L * 1024 * 1024
        private const val MAX_SHARE_INBOX_BYTES = 512L * 1024 * 1024
        private const val COPY_BUFFER_BYTES = 64 * 1024
    }

    private val pending = ArrayDeque<IncomingShare>()
    private val pendingErrors = ArrayDeque<String>()
    private val captureExecutor = Executors.newSingleThreadExecutor()
    private var lastIntent: Intent? = null

    override fun onNewIntent(intent: Intent) {
        enqueueCapture(intent)
    }

    override fun onResume() {
        enqueueCapture(activity.intent)
    }

    @Command
    fun takePendingShare(invoke: Invoke) {
        enqueueCapture(activity.intent)
        captureExecutor.execute {
            val (error, value) = synchronized(this) {
                Pair(
                    if (pendingErrors.isEmpty()) null else pendingErrors.removeFirst(),
                    if (pending.isEmpty()) null else pending.removeFirst(),
                )
            }
            activity.runOnUiThread {
                if (error != null) invoke.reject(error)
                else if (value == null) invoke.resolve()
                else invoke.resolveObject(value)
            }
        }
    }

    @Command
    fun shareFile(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(ShareFileArgs::class.java)
            val file = File(args.path).canonicalFile
            val exportRoot = File(activity.cacheDir, "workspace-exports").canonicalFile
            require(file.isFile && file.path.startsWith(exportRoot.path + File.separator)) {
                "The exported file is outside Marktree's sharing directory."
            }
            val uri = FileProvider.getUriForFile(
                activity,
                "${activity.packageName}.fileprovider",
                file,
            )
            val send = Intent(Intent.ACTION_SEND).apply {
                type = args.mediaType
                putExtra(Intent.EXTRA_STREAM, uri)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            activity.startActivity(Intent.createChooser(send, args.title))
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message)
        }
    }

    private fun enqueueCapture(intent: Intent?) {
        if (intent == null) return
        synchronized(this) {
            if (intent === lastIntent) return
            lastIntent = intent
        }
        val snapshot = Intent(intent)
        captureExecutor.execute {
            try {
                capture(snapshot)
            } catch (error: Exception) {
                synchronized(this) {
                    pendingErrors.addLast(error.message ?: "Could not receive the shared content.")
                }
            }
        }
    }

    private fun capture(intent: Intent) {
        if (intent.action != Intent.ACTION_SEND && intent.action != Intent.ACTION_VIEW) return
        val uri = when (intent.action) {
            Intent.ACTION_VIEW -> intent.data
            else -> incomingStream(intent)
        }
        val copied = uri?.let(::copyIncomingUri)
        val text = if (intent.action == Intent.ACTION_SEND) {
            intent.getStringExtra(Intent.EXTRA_TEXT)
        } else null
        if (copied == null && text.isNullOrBlank()) return
        synchronized(this) {
            pending.addLast(IncomingShare(
                text = text,
                subject = intent.getStringExtra(Intent.EXTRA_SUBJECT),
                filePath = copied?.first?.absolutePath,
                fileName = copied?.second,
                mediaType = intent.type ?: uri?.let(activity.contentResolver::getType),
            ))
        }
    }

    private fun copyIncomingUri(uri: Uri): Pair<File, String> {
        val displayName = activity.contentResolver.query(
            uri,
            arrayOf(OpenableColumns.DISPLAY_NAME),
            null,
            null,
            null,
        )?.use { cursor ->
            if (cursor.moveToFirst()) cursor.getString(0) else null
        } ?: "shared-file"
        val safeName = displayName.replace(Regex("[\\\\/:*?\"<>|]"), "-")
        val directory = File(activity.cacheDir, "share-inbox/${UUID.randomUUID()}")
        check(directory.mkdirs()) { "Could not create the share inbox." }
        try {
            val target = File(directory, safeName).canonicalFile
            check(target.path.startsWith(directory.canonicalPath + File.separator)) {
                "The shared file name is invalid."
            }
            activity.contentResolver.openInputStream(uri).use { input ->
                checkNotNull(input) { "Could not read the shared file." }
                target.outputStream().use { output ->
                    val buffer = ByteArray(COPY_BUFFER_BYTES)
                    var total = 0L
                    while (true) {
                        val count = input.read(buffer)
                        if (count < 0) break
                        total += count
                        require(total <= MAX_INCOMING_SHARE_BYTES) {
                            "The shared file exceeds Marktree's 256 MB import limit."
                        }
                        output.write(buffer, 0, count)
                    }
                }
            }
            enforceInboxBudget(directory)
            return target to safeName
        } catch (error: Exception) {
            directory.deleteRecursively()
            throw error
        }
    }

    private fun enforceInboxBudget(current: File) {
        val inbox = File(activity.cacheDir, "share-inbox")
        val protected = synchronized(this) {
            pending
                .mapNotNull { share -> share.filePath }
                .map { path -> File(path).parentFile?.canonicalPath }
                .filterNotNull()
                .toSet()
        }
        val candidates = inbox.listFiles()
            ?.filter { directory ->
                directory.canonicalPath != current.canonicalPath &&
                    directory.canonicalPath !in protected
            }
            ?.sortedBy(File::lastModified)
            .orEmpty()
        var total = directorySize(inbox)
        for (candidate in candidates) {
            if (total <= MAX_SHARE_INBOX_BYTES) break
            val bytes = directorySize(candidate)
            if (candidate.deleteRecursively()) total -= bytes
        }
        require(total <= MAX_SHARE_INBOX_BYTES) {
            "Marktree's share inbox is full. Import or retry the pending shared files first."
        }
    }

    private fun directorySize(file: File): Long {
        if (file.isFile) return file.length()
        return file.listFiles()?.fold(0L) { total, child ->
            total + directorySize(child)
        } ?: 0L
    }

    @Suppress("DEPRECATION")
    private fun incomingStream(intent: Intent): Uri? =
        intent.getParcelableExtra(Intent.EXTRA_STREAM)
}

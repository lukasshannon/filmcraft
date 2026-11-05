package app.filmcraft.android;

import android.app.NativeActivity;
import android.content.ClipData;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.provider.OpenableColumns;
import android.provider.DocumentsContract;
import android.view.View;
import android.view.WindowInsets;
import android.view.RoundedCorner;
import android.view.WindowManager;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Platform file access and display insets. The complete editor and rendering run in Rust. */
public final class FilmCraftActivity extends NativeActivity {
    private static final int PICK = 101;
    private static final long IMPORT_LIMIT = 512L * 1024 * 1024;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private File bridge;
    private String pendingName;
    private JSONObject pendingRequest;
    private boolean destroyed;
    private String lastInsets;

    private final Runnable poll = new Runnable() {
        @Override public void run() {
            if (destroyed) return;
            File[] requests = new File(bridge, "requests").listFiles((dir, name) -> name.endsWith(".json"));
            if (requests != null) {
                Arrays.sort(requests);
                for (File request : requests) {
                    try {
                        JSONObject value = readJson(request);
                        String kind = value.optString("kind");
                        if ((kind.equals("open") || kind.equals("save") || kind.equals("folder")) && pendingName != null) continue;
                        if (!request.delete()) continue;
                        dispatch(request.getName(), value);
                    } catch (Exception error) {
                        request.delete();
                        error(request.getName(), error);
                    }
                }
            }
            handler.postDelayed(this, 120);
        }
    };

    @Override public void onCreate(Bundle state) {
        bridge = new File(getFilesDir(), "bridge");
        new File(bridge, "requests").mkdirs();
        new File(bridge, "replies").mkdirs();
        if (state != null) {
            pendingName = state.getString("pickerName");
            try { String text = state.getString("pickerRequest"); if (text != null) pendingRequest = new JSONObject(text); }
            catch (Exception ignored) { pendingName = null; }
        }
        super.onCreate(state);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_FULLSCREEN);
        if (Build.VERSION.SDK_INT >= 30) getWindow().setDecorFitsSystemWindows(false);
        View decor = getWindow().getDecorView();
        decor.setOnApplyWindowInsetsListener((view, insets) -> {
            publishInsets(view, insets);
            return insets;
        });
        decor.addOnLayoutChangeListener((view, left, top, right, bottom, oldLeft, oldTop, oldRight, oldBottom) -> {
            WindowInsets insets = view.getRootWindowInsets();
            if (insets != null) publishInsets(view, insets);
        });
        decor.requestApplyInsets();
        immersive();
        handler.post(poll);
    }

    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused) immersive();
    }

    private void immersive() {
        getWindow().getDecorView().setSystemUiVisibility(View.SYSTEM_UI_FLAG_FULLSCREEN
            | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
            | View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
            | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
    }

    private void publishInsets(View view, WindowInsets insets) {
        int width = view.getWidth(), height = view.getHeight();
        if (width <= 0 || height <= 0) return;
        // Immersive chrome and gesture regions do not reserve empty strips around the editor.
        // Only the keyboard shrinks its usable rectangle. Corner padding stays local.
        int bottom = 0;
        if (Build.VERSION.SDK_INT >= 30) {
            bottom = insets.getInsets(WindowInsets.Type.ime()).bottom;
        } else {
            int lower = insets.getSystemWindowInsetBottom();
            if (lower > 100 * getResources().getDisplayMetrics().density) bottom = lower;
        }
        int[] location = new int[2];
        view.getLocationInWindow(location);
        JSONArray corners = new JSONArray();
        try {
        if (Build.VERSION.SDK_INT >= 31) {
            for (int position = 0; position < 4; position++) {
                RoundedCorner corner = insets.getRoundedCorner(position);
                if (corner == null) continue;
                corners.put(new JSONObject().put("position", position).put("radius", corner.getRadius())
                    .put("x", corner.getCenter().x - location[0]).put("y", corner.getCenter().y - location[1]));
            }
        }
            String value = new JSONObject().put("width", width).put("height", height)
                .put("left", 0).put("top", 0).put("right", 0).put("bottom", bottom)
                .put("corners", corners).toString();
            if (value.equals(lastInsets)) return;
            File temporary = new File(bridge, "safe-area.tmp");
            Files.write(temporary.toPath(), value.getBytes(StandardCharsets.UTF_8));
            Files.move(temporary.toPath(), new File(bridge, "safe-area.json").toPath(), StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            lastInsets = value;
        } catch (Exception error) {
            android.util.Log.w("FilmCraft", "Cannot update display safe area", error);
        }
    }

    @Override protected void onSaveInstanceState(Bundle state) {
        if (pendingName != null) state.putString("pickerName", pendingName);
        if (pendingRequest != null) state.putString("pickerRequest", pendingRequest.toString());
        super.onSaveInstanceState(state);
    }

    @Override public void onDestroy() {
        destroyed = true;
        handler.removeCallbacks(poll);
        worker.shutdown();
        super.onDestroy();
    }

    private void dispatch(String name, JSONObject request) throws Exception {
        String kind = request.getString("kind");
        // The emulator package can deliver a scoped, instrumented platform result
        // without synthetic OS input. Javac removes this branch from ARM releases.
        if (BuildConfig.INSTRUMENTED && kind.equals("test-result")) {
            if (pendingName == null || pendingRequest == null) throw new java.io.IOException("No pending system picker");
            if (!pendingRequest.getString("kind").equals(request.getString("expected"))) throw new java.io.IOException("Unexpected picker kind");
            Intent answer = new Intent();
            if (request.has("uri")) answer.setData(Uri.parse(request.getString("uri")));
            answer.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
            onActivityResult(PICK, request.optBoolean("cancel") ? RESULT_CANCELED : RESULT_OK, answer);
            finishActivity(PICK);
            reply(name, new JSONObject().put("kind", "instrumented-result"));
            return;
        }
        if (kind.equals("write")) {
            worker.execute(() -> {
                try {
                    File file = new File(request.getString("path")).getCanonicalFile();
                    File exports = new File(getFilesDir(), "exports").getCanonicalFile();
                    if (!file.toPath().startsWith(exports.toPath())) throw new SecurityException("Invalid export path");
                    Uri uri = outputDocument(file, exports);
                    try (InputStream input = new FileInputStream(file);
                         OutputStream output = getContentResolver().openOutputStream(uri, "wt")) {
                        if (output == null) throw new java.io.IOException("The document provider could not open this file for writing");
                        copy(input, output, Long.MAX_VALUE);
                        output.flush();
                    }
                    reply(name, new JSONObject().put("kind", "written"));
                } catch (Exception failure) { error(name, failure); }
            });
            return;
        }
        if (!kind.equals("open") && !kind.equals("save") && !kind.equals("folder")) throw new java.io.IOException("Unknown Android request");
        Intent intent = new Intent(kind.equals("folder") ? Intent.ACTION_OPEN_DOCUMENT_TREE : kind.equals("open") ? Intent.ACTION_OPEN_DOCUMENT : Intent.ACTION_CREATE_DOCUMENT);
        if (!kind.equals("folder")) intent.addCategory(Intent.CATEGORY_OPENABLE);
        intent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        if (kind.equals("open")) {
            intent.setType("*/*");
            intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, request.optBoolean("multiple"));
        } else if (kind.equals("save")) {
            String suggested = safeName(request.optString("suggested", "Photo.jpg"));
            intent.setType(mime(suggested));
            intent.putExtra(Intent.EXTRA_TITLE, suggested);
            intent.addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        } else {
            intent.addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION | Intent.FLAG_GRANT_PREFIX_URI_PERMISSION);
        }
        pendingName = name;
        pendingRequest = request;
        try { startActivityForResult(intent, PICK); }
        catch (Exception failure) { pendingName = null; pendingRequest = null; throw failure; }
    }

    @Override protected void onActivityResult(int code, int result, Intent data) {
        super.onActivityResult(code, result, data);
        if (code != PICK || pendingName == null || pendingRequest == null) return;
        final String name = pendingName;
        final JSONObject request = pendingRequest;
        pendingName = null;
        pendingRequest = null;
        if (result != RESULT_OK || data == null) {
            try { reply(name, new JSONObject().put("kind", "cancelled")); }
            catch (Exception failure) { error(name, failure); }
            return;
        }
        worker.execute(() -> {
            try {
                if (request.getString("kind").equals("folder")) {
                    Uri tree = data.getData();
                    if (tree == null) throw new java.io.IOException("No folder selected");
                    if (!(BuildConfig.INSTRUMENTED && "app.filmcraft.android.test.documents".equals(tree.getAuthority())))
                        getContentResolver().takePersistableUriPermission(tree, data.getFlags() & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION));
                    boolean export = request.optBoolean("export");
                    File directory = new File(getFilesDir(), (export ? "exports/" : "imports/") + name);
                    if (!directory.mkdirs() && !directory.isDirectory()) throw new java.io.IOException("Cannot prepare selected folder");
                    if (export) {
                        // The engine's collision policy checks local paths. Mirror names
                        // (never contents), so Rename/Skip/Replace see existing documents.
                        JSONObject known = new JSONObject();
                        Uri document = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree));
                        snapshotNames(tree, document, directory, directory, known, new int[]{0}, 0);
                        writeTreeMap(directory, known);
                        Files.write(new File(directory, ".tree-uri").toPath(), tree.toString().getBytes(StandardCharsets.UTF_8));
                    } else {
                        Uri document = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree));
                        copyTree(tree, document, directory, new long[]{0, 0}, 0);
                    }
                    reply(name, new JSONObject().put("kind", "paths").put("paths", new JSONArray().put(directory.getAbsolutePath())));
                } else if (request.getString("kind").equals("save")) {
                    Uri uri = data.getData();
                    if (uri == null) throw new java.io.IOException("No save destination was selected");
                    try { getContentResolver().takePersistableUriPermission(uri, data.getFlags() & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION)); }
                    catch (SecurityException ignored) { /* Transient provider grants still allow this session's save. */ }
                    File directory = new File(getFilesDir(), "exports/" + name);
                    if (!directory.mkdirs() && !directory.isDirectory()) throw new java.io.IOException("Cannot create export directory");
                    String suggested = safeName(request.optString("suggested", "Photo.jpg"));
                    String selected = displayName(uri, suggested);
                    // The editor selects its encoder from the chosen name, including a changed extension.
                    if (selected.lastIndexOf('.') < 0 && suggested.lastIndexOf('.') >= 0)
                        selected += suggested.substring(suggested.lastIndexOf('.'));
                    File destination = new File(directory, selected);
                    Files.write(new File(destination.getPath() + ".uri").toPath(), uri.toString().getBytes(StandardCharsets.UTF_8));
                    reply(name, new JSONObject().put("kind", "save").put("path", destination.getAbsolutePath()));
                } else {
                    ArrayList<Uri> uris = new ArrayList<>();
                    ClipData clip = data.getClipData();
                    if (clip != null) for (int index = 0; index < clip.getItemCount(); index++) uris.add(clip.getItemAt(index).getUri());
                    else if (data.getData() != null) uris.add(data.getData());
                    if (uris.isEmpty()) throw new java.io.IOException("No readable files were selected");
                    JSONArray paths = new JSONArray();
                    long total = 0;
                    for (int index = 0; index < uris.size(); index++) {
                        Uri uri = uris.get(index);
                        String filename = displayName(uri, "image.png");
                        File directory = new File(getFilesDir(), "imports/" + name + "/" + index);
                        if (!directory.mkdirs() && !directory.isDirectory()) throw new java.io.IOException("Cannot create import directory");
                        File local = new File(directory, filename);
                        try (InputStream input = getContentResolver().openInputStream(uri);
                             OutputStream output = new FileOutputStream(local)) {
                            if (input == null) throw new java.io.IOException("The selected document could not be read");
                            total += copy(input, output, IMPORT_LIMIT - total);
                        }
                        paths.put(local.getAbsolutePath());
                    }
                    reply(name, new JSONObject().put("kind", "paths").put("paths", paths));
                }
            } catch (Exception failure) { error(name, failure); }
        });
    }

    private String displayName(Uri uri, String fallback) {
        try (Cursor cursor = getContentResolver().query(uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cursor != null && cursor.moveToFirst()) return safeName(cursor.getString(0));
        } catch (Exception ignored) { }
        return safeName(fallback);
    }

    /** Resolve a private export path against only the tree the user granted. */
    private Uri outputDocument(File file, File exports) throws Exception {
        File exact = new File(file.getPath() + ".uri");
        if (exact.isFile()) return Uri.parse(new String(Files.readAllBytes(exact.toPath()), StandardCharsets.UTF_8));
        File directory = file.getParentFile();
        while (directory != null && directory.toPath().startsWith(exports.toPath())) {
            File map = new File(directory, ".tree-uri");
            if (map.isFile()) {
                Uri tree = Uri.parse(new String(Files.readAllBytes(map.toPath()), StandardCharsets.UTF_8));
                JSONObject known = new JSONObject(new String(Files.readAllBytes(treeMap(directory).toPath()), StandardCharsets.UTF_8));
                String relative = directory.toPath().relativize(file.toPath()).toString();
                if (known.has(relative)) return Uri.parse(known.getString(relative));
                Uri parent = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree));
                String[] segments = relative.split("/");
                for (int index = 0; index < segments.length; index++) {
                    String segment = safeName(segments[index]);
                    boolean last = index == segments.length - 1;
                    // New files use createDocument, which must create a document;
                    // another app racing us cannot turn this into an overwrite.
                    Uri child = last ? null : findChild(tree, parent, segment);
                    if (child == null) child = DocumentsContract.createDocument(getContentResolver(), parent,
                        last ? mime(segment) : DocumentsContract.Document.MIME_TYPE_DIR, segment);
                    if (child == null) throw new java.io.IOException("The document provider could not create " + segment);
                    parent = child;
                }
                known.put(relative, parent.toString());
                writeTreeMap(directory, known);
                return parent;
            }
            directory = directory.getParentFile();
        }
        throw new java.io.IOException("Choose an Android export folder first");
    }

    private File treeMap(File directory) throws Exception {
        File maps = new File(bridge, "tree-maps");
        if (!maps.mkdirs() && !maps.isDirectory()) throw new java.io.IOException("Cannot prepare export mapping");
        return new File(maps, directory.getName() + ".json");
    }

    private void writeTreeMap(File directory, JSONObject known) throws Exception {
        File target = treeMap(directory);
        File temporary = new File(target.getPath() + ".tmp");
        Files.write(temporary.toPath(), known.toString().getBytes(StandardCharsets.UTF_8));
        Files.move(temporary.toPath(), target.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
    }

    private void snapshotNames(Uri tree, Uri parent, File local, File base, JSONObject known, int[] count, int depth) throws Exception {
        if (depth > 24) throw new java.io.IOException("Choose an export folder nested at most 24 levels");
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getDocumentId(parent));
        try (Cursor cursor = getContentResolver().query(children, new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE}, null, null, null)) {
            if (cursor == null) throw new java.io.IOException("Cannot inspect the selected export folder");
            while (cursor.moveToNext()) {
                if (++count[0] > 5000) throw new java.io.IOException("Choose an export folder with at most 5000 files and folders");
                String name = safeName(cursor.getString(1));
                if (name.equals(".tree-uri")) continue;
                File child = new File(local, name);
                Uri document = DocumentsContract.buildDocumentUriUsingTree(tree, cursor.getString(0));
                if (DocumentsContract.Document.MIME_TYPE_DIR.equals(cursor.getString(2))) {
                    if (!child.mkdir() && !child.isDirectory()) throw new java.io.IOException("Cannot prepare export folder");
                    snapshotNames(tree, document, child, base, known, count, depth + 1);
                } else {
                    if (!child.createNewFile()) throw new java.io.IOException("Two documents have the same export name");
                    known.put(base.toPath().relativize(child.toPath()).toString(), document.toString());
                }
            }
        }
    }

    private Uri findChild(Uri tree, Uri parent, String name) throws Exception {
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getDocumentId(parent));
        try (Cursor cursor = getContentResolver().query(children, new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME}, null, null, null)) {
            if (cursor == null) throw new java.io.IOException("The document provider could not list this folder");
            while (cursor.moveToNext()) if (name.equals(cursor.getString(1)))
                return DocumentsContract.buildDocumentUriUsingTree(tree, cursor.getString(0));
        }
        return null;
    }

    private void copyTree(Uri tree, Uri parent, File local, long[] budget, int depth) throws Exception {
        if (depth > 24) throw new java.io.IOException("The selected folder is nested too deeply");
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getDocumentId(parent));
        try (Cursor cursor = getContentResolver().query(children, new String[]{DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE}, null, null, null)) {
            if (cursor == null) throw new java.io.IOException("The document provider could not list this folder");
            while (cursor.moveToNext()) {
                if (++budget[1] > 5000) throw new java.io.IOException("Select at most 5000 files and folders at a time");
                String filename = safeName(cursor.getString(1));
                File child = new File(local, filename);
                Uri document = DocumentsContract.buildDocumentUriUsingTree(tree, cursor.getString(0));
                if (DocumentsContract.Document.MIME_TYPE_DIR.equals(cursor.getString(2))) {
                    if (!child.mkdir()) throw new java.io.IOException("Cannot create import folder");
                    copyTree(tree, document, child, budget, depth + 1);
                } else {
                    try (InputStream input = getContentResolver().openInputStream(document); OutputStream output = new FileOutputStream(child)) {
                        if (input == null) throw new java.io.IOException("Cannot read " + filename);
                        budget[0] += copy(input, output, IMPORT_LIMIT - budget[0]);
                    }
                }
            }
        }
    }

    private static String safeName(String value) {
        if (value == null) return "Photo.jpg";
        value = value.replace('\\', '_').replace('/', '_');
        if (value.equals(".") || value.equals("..") || value.trim().isEmpty()) return "Photo.jpg";
        int dot = value.lastIndexOf('.');
        if (dot >= 0) value = value.substring(0, dot) + value.substring(dot).toLowerCase(java.util.Locale.ROOT);
        return value;
    }

    private static String mime(String filename) {
        String extension = filename.substring(filename.lastIndexOf('.') + 1).toLowerCase(java.util.Locale.ROOT);
        if (extension.equals("jpg") || extension.equals("jpeg")) return "image/jpeg";
        if (extension.equals("png")) return "image/png";
        if (extension.equals("webp")) return "image/webp";
        if (extension.equals("tif") || extension.equals("tiff")) return "image/tiff";
        if (extension.equals("avif")) return "image/avif";
        if (extension.equals("pdf")) return "application/pdf";
        return "application/octet-stream";
    }

    private static long copy(InputStream input, OutputStream output, long limit) throws Exception {
        byte[] buffer = new byte[64 * 1024];
        long total = 0;
        int count;
        while ((count = input.read(buffer)) != -1) {
            total += count;
            if (total > limit) throw new java.io.IOException("Select at most 512 MiB of files at a time on Android");
            output.write(buffer, 0, count);
        }
        return total;
    }

    private static JSONObject readJson(File file) throws Exception {
        if (file.length() > 65536) throw new java.io.IOException("Android request is too large");
        return new JSONObject(new String(Files.readAllBytes(file.toPath()), StandardCharsets.UTF_8));
    }

    private void reply(String name, JSONObject value) throws Exception {
        File target = new File(bridge, "replies/" + name);
        File temporary = new File(target.getPath() + ".tmp");
        Files.write(temporary.toPath(), value.toString().getBytes(StandardCharsets.UTF_8));
        Files.move(temporary.toPath(), target.toPath(), StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
    }

    private void error(String name, Exception failure) {
        try { reply(name, new JSONObject().put("kind", "error").put("message", failure.getMessage() == null ? failure.toString() : failure.getMessage())); }
        catch (Exception nested) { android.util.Log.e("FilmCraft", "Cannot deliver Android file result", nested); }
    }
}

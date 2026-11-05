package app.filmcraft.android;

import android.database.Cursor;
import android.database.MatrixCursor;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract.Document;
import android.provider.DocumentsContract.Root;
import android.provider.DocumentsProvider;
import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/** Private DocumentsProvider fixture, packaged only into the x86 emulator APK. */
public final class TestDocumentsProvider extends DocumentsProvider {
    private File root;
    @Override public boolean onCreate() {
        root = new File(getContext().getFilesDir(), "control/test-documents");
        try { root = root.getCanonicalFile(); } catch (IOException e) { return false; }
        return root.mkdirs() || root.isDirectory();
    }
    private File file(String id) throws FileNotFoundException {
        try {
            if (!id.equals("root") && !id.startsWith("root/")) throw new IOException("Invalid document");
            File file = new File(root, id.equals("root") ? "" : id.substring(5)).getCanonicalFile();
            if (!file.toPath().startsWith(root.getCanonicalFile().toPath())) throw new IOException("Invalid path");
            return file;
        } catch (IOException e) { throw new FileNotFoundException(e.toString()); }
    }
    private String id(File file) { return file.equals(root) ? "root" : "root/" + root.toPath().relativize(file.toPath()); }
    private void add(MatrixCursor cursor, File file) {
        MatrixCursor.RowBuilder row = cursor.newRow();
        for (String column : cursor.getColumnNames()) {
            Object value = null;
            if (column.equals(Document.COLUMN_DOCUMENT_ID)) value = id(file);
            if (column.equals(Document.COLUMN_DISPLAY_NAME)) value = file.getName();
            if (column.equals(Document.COLUMN_MIME_TYPE)) value = file.isDirectory() ? Document.MIME_TYPE_DIR : file.getName().endsWith(".png") ? "image/png" : "application/octet-stream";
            if (column.equals(Document.COLUMN_SIZE)) value = file.length();
            if (column.equals(Document.COLUMN_LAST_MODIFIED)) value = file.lastModified();
            if (column.equals(Document.COLUMN_FLAGS)) value = file.isDirectory() ? Document.FLAG_DIR_SUPPORTS_CREATE : Document.FLAG_SUPPORTS_WRITE;
            row.add(value);
        }
    }
    private static String[] columns(String[] projection) {
        return projection == null ? new String[]{Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE, Document.COLUMN_SIZE, Document.COLUMN_LAST_MODIFIED, Document.COLUMN_FLAGS} : projection;
    }
    @Override public Cursor queryRoots(String[] projection) {
        return new MatrixCursor(projection == null ? new String[]{Root.COLUMN_ROOT_ID, Root.COLUMN_DOCUMENT_ID, Root.COLUMN_TITLE} : projection);
    }
    @Override public Cursor queryDocument(String documentId, String[] projection) throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(columns(projection)); add(cursor, file(documentId)); return cursor;
    }
    @Override public Cursor queryChildDocuments(String documentId, String[] projection, String sortOrder) throws FileNotFoundException {
        MatrixCursor cursor = new MatrixCursor(columns(projection));
        File[] children = file(documentId).listFiles();
        if (children != null) for (File child : children) add(cursor, child);
        return cursor;
    }
    @Override public String createDocument(String parentId, String mimeType, String displayName) throws FileNotFoundException {
        if (displayName.contains("/") || displayName.equals(".") || displayName.equals("..")) throw new FileNotFoundException("Invalid name");
        File next = new File(file(parentId), displayName);
        try {
            if (!(Document.MIME_TYPE_DIR.equals(mimeType) ? next.mkdir() : next.createNewFile())) throw new IOException("Already exists");
            return id(next);
        } catch (IOException e) { throw new FileNotFoundException(e.toString()); }
    }
    @Override public boolean isChildDocument(String parentId, String childId) {
        try { return !parentId.equals(childId) && file(childId).toPath().startsWith(file(parentId).toPath()); }
        catch (FileNotFoundException e) { return false; }
    }
    @Override public ParcelFileDescriptor openDocument(String documentId, String mode, CancellationSignal signal) throws FileNotFoundException {
        return ParcelFileDescriptor.open(file(documentId), ParcelFileDescriptor.parseMode(mode));
    }
}

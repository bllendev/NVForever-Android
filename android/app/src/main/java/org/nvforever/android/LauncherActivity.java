package org.nvforever.android;

import android.Manifest;
import android.app.Activity;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.graphics.Color;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.provider.DocumentsContract;
import android.provider.Settings;
import android.view.View;
import android.widget.Button;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Switch;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

/**
 * The screen the app opens on. It does the three things only Android's Java side can do:
 * ask for "all files access" (so the game can read and write any folder of the phone, like on
 * a PC), let the player pick the Fallout: New Vegas Data folder anywhere with Android's own
 * folder picker (or create the default one), and start the game. The chosen folder is written to
 * files/data-path.txt, which the Rust side (nvf-android/src/paths.rs) reads.
 *
 * Folders under Android/data and Android/obb of other apps cannot be used: Android closes them
 * to every app since Android 11, with or without the permission.
 */
public class LauncherActivity extends Activity {
    private static final int REQUEST_TREE = 2;
    private static final int REQUEST_LEGACY_PERMISSION = 1;

    private TextView status;
    private Switch controllerSwitch;
    private Switch touchSwitch;
    private boolean loadingControls;
    private Button permissionButton;
    private Button pickButton;
    private Button defaultButton;
    private Button startButton;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        float d = getResources().getDisplayMetrics().density;
        int pad = (int) (20 * d);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(Color.rgb(30, 31, 34));
        root.setPadding(pad, (int) (56 * d), pad, (int) (40 * d));

        TextView title = new TextView(this);
        title.setText("NVForever");
        title.setTextColor(Color.WHITE);
        title.setTextSize(26);
        root.addView(title);

        status = new TextView(this);
        status.setTextColor(Color.rgb(220, 220, 220));
        status.setTextSize(15);
        status.setPadding(0, pad / 2, 0, pad / 2);
        root.addView(status);

        controllerSwitch = new Switch(this);
        controllerSwitch.setText("Mando físico (Retroid / Xbox)");
        controllerSwitch.setTextColor(Color.WHITE);
        root.addView(controllerSwitch);
        touchSwitch = new Switch(this);
        touchSwitch.setText("Mostrar controles táctiles");
        touchSwitch.setTextColor(Color.WHITE);
        root.addView(touchSwitch);
        TextView controlsHint = new TextView(this);
        controlsHint.setText("Los cambios se aplican al iniciar el juego.\nMando: A usar/aceptar · B Pip-Boy/volver · X recargar · Y saltar\nLT apuntar · RT disparar · RB VATS · LB cámara · Start pausa");
        controlsHint.setTextColor(Color.LTGRAY);
        controlsHint.setPadding(0, pad / 2, 0, pad / 2);
        root.addView(controlsHint);
        controllerSwitch.setOnCheckedChangeListener((b, checked) -> saveControls());
        touchSwitch.setOnCheckedChangeListener((b, checked) -> saveControls());

        permissionButton = addButton(root, "1. Conceder acesso a todos os arquivos", v -> requestFilesAccess());
        pickButton = addButton(root, "2. Escolher a pasta Data do Fallout", v -> pickFolder());
        defaultButton = addButton(root, "Ou criar a pasta padrao (NVForever/Data)", v -> createDefaultFolder());
        startButton = addButton(root, "3. Iniciar o jogo", v -> startGame());

        ScrollView scroll = new ScrollView(this);
        scroll.setBackgroundColor(Color.rgb(30, 31, 34));
        scroll.addView(root);
        setContentView(scroll);
    }

    private Button addButton(LinearLayout parent, String label, View.OnClickListener listener) {
        Button b = new Button(this);
        b.setText(label);
        b.setAllCaps(false);
        b.setOnClickListener(listener);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                LinearLayout.LayoutParams.MATCH_PARENT, LinearLayout.LayoutParams.WRAP_CONTENT);
        lp.topMargin = (int) (10 * getResources().getDisplayMetrics().density);
        parent.addView(b, lp);
        return b;
    }

    @Override
    protected void onResume() {
        super.onResume();
        refresh();
        resumeRequested(getIntent());
    }

    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        resumeRequested(intent);
    }

    private void resumeRequested(Intent intent) {
        if (!intent.getBooleanExtra("resumeGame", false)) return;
        intent.removeExtra("resumeGame");
        File request = new File(getFilesDir(), "NVForever/android-restart.json");
        if (!request.isFile()) return;
        try {
            byte[] bytes = java.nio.file.Files.readAllBytes(request.toPath());
            org.json.JSONObject packet = new org.json.JSONObject(new String(bytes, StandardCharsets.UTF_8));
            int pid = packet.getInt("pid");
            org.json.JSONArray args = packet.getJSONArray("args");
            if (pid <= 0 || pid == android.os.Process.myPid() || args.length() == 0) throw new IOException("Invalid restart request");
            File next = new File(getFilesDir(), "android-next-run.json");
            File temp = new File(getFilesDir(), "android-next-run.tmp");
            java.nio.file.Files.write(temp.toPath(), args.toString().getBytes(StandardCharsets.UTF_8));
            java.nio.file.Files.move(temp.toPath(), next.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            if (!request.delete()) throw new IOException("Cannot consume restart request");
            // Only end our own game process, identified by both UID and process name.
            android.app.ActivityManager manager = (android.app.ActivityManager) getSystemService(ACTIVITY_SERVICE);
            java.util.List<android.app.ActivityManager.RunningAppProcessInfo> processes = manager.getRunningAppProcesses();
            if (processes != null) for (android.app.ActivityManager.RunningAppProcessInfo process : processes) {
                if (process.pid == pid && process.uid == android.os.Process.myUid()
                        && process.processName.equals(getPackageName() + ":game")) {
                    android.os.Process.killProcess(pid);
                }
            }
            final long deadline = android.os.SystemClock.uptimeMillis() + 5000;
            android.os.Handler handler = new android.os.Handler(android.os.Looper.getMainLooper());
            handler.postDelayed(new Runnable() {
                @Override public void run() {
                    boolean alive = false;
                    java.util.List<android.app.ActivityManager.RunningAppProcessInfo> running = manager.getRunningAppProcesses();
                    if (running != null) for (android.app.ActivityManager.RunningAppProcessInfo process : running) {
                        if (process.pid == pid && process.uid == android.os.Process.myUid()
                                && process.processName.equals(getPackageName() + ":game")) alive = true;
                    }
                    if (!alive) startActivity(new Intent(LauncherActivity.this, GameActivity.class).addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT | Intent.FLAG_ACTIVITY_SINGLE_TOP));
                    else if (android.os.SystemClock.uptimeMillis() < deadline) handler.postDelayed(this, 100);
                    else {
                        next.delete();
                        android.widget.Toast.makeText(LauncherActivity.this, "No se pudo reiniciar el juego", android.widget.Toast.LENGTH_LONG).show();
                    }
                }
            }, 250);
        } catch (Exception e) {
            android.util.Log.e("NVForever", "Restarting game", e);
            android.widget.Toast.makeText(this, "No se pudo cargar: " + e, android.widget.Toast.LENGTH_LONG).show();
        }
    }

    private File controlsFile() {
        return new File(Environment.getExternalStorageDirectory(), "NVForever/nvforever-controls.txt");
    }

    private void loadControls() {
        boolean controller = false;
        boolean touch = true;
        if (hasFilesAccess() && controlsFile().isFile()) {
            try (java.io.BufferedReader in = new java.io.BufferedReader(new java.io.FileReader(controlsFile()))) {
                String line;
                while ((line = in.readLine()) != null) {
                    String[] pair = line.trim().split("\\s+");
                    if (pair.length < 2) continue;
                    if (!pair[1].equals("true") && !pair[1].equals("false") && !pair[1].equals("1") && !pair[1].equals("0")) continue;
                    boolean on = pair[1].equals("true") || pair[1].equals("1");
                    if (pair[0].equals("controller")) controller = on;
                    if (pair[0].equals("touch")) touch = on;
                }
            } catch (IOException e) { android.util.Log.e("NVForever", "Reading controls", e); }
        }
        loadingControls = true;
        controllerSwitch.setChecked(controller);
        touchSwitch.setChecked(touch);
        loadingControls = false;
    }

    private void saveControls() {
        if (loadingControls) return;
        if (!hasFilesAccess()) { loadControls(); return; }
        File file = controlsFile();
        file.getParentFile().mkdirs();
        File temp = new File(file.getPath() + ".tmp");
        try {
            try (FileOutputStream out = new FileOutputStream(temp)) {
                String text = "# Applied when starting the game\ncontroller " + controllerSwitch.isChecked()
                        + "\ntouch " + touchSwitch.isChecked() + "\n";
                out.write(text.getBytes(StandardCharsets.UTF_8));
            }
            if (!temp.renameTo(file)) throw new IOException("Cannot replace controls config");
        } catch (IOException e) {
            android.widget.Toast.makeText(this, "No se pudo guardar: " + e, android.widget.Toast.LENGTH_LONG).show();
            loadControls();
        }
    }

    // ---- permission -------------------------------------------------------------------

    private boolean hasFilesAccess() {
        if (Build.VERSION.SDK_INT >= 30) {
            return Environment.isExternalStorageManager();
        }
        return checkSelfPermission(Manifest.permission.WRITE_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED;
    }

    private void requestFilesAccess() {
        if (Build.VERSION.SDK_INT >= 30) {
            try {
                Intent i = new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                        Uri.parse("package:" + getPackageName()));
                startActivity(i);
            } catch (Exception e) {
                startActivity(new Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION));
            }
        } else {
            requestPermissions(new String[] {
                    Manifest.permission.READ_EXTERNAL_STORAGE,
                    Manifest.permission.WRITE_EXTERNAL_STORAGE
            }, REQUEST_LEGACY_PERMISSION);
        }
    }

    @Override
    public void onRequestPermissionsResult(int requestCode, String[] permissions, int[] grantResults) {
        super.onRequestPermissionsResult(requestCode, permissions, grantResults);
        refresh();
    }

    // ---- the chosen folder ------------------------------------------------------------

    private File chosenFile() {
        return new File(getFilesDir(), "data-path.txt");
    }

    private File defaultDataDir() {
        return new File(Environment.getExternalStorageDirectory(), "NVForever/Data");
    }

    /** The folder the game will use: the chosen one, else the default one. */
    private File dataDir() {
        File f = chosenFile();
        if (f.isFile()) {
            try {
                byte[] bytes = new byte[(int) Math.min(f.length(), 4096)];
                java.io.FileInputStream in = new java.io.FileInputStream(f);
                int n = in.read(bytes);
                in.close();
                String line = new String(bytes, 0, Math.max(n, 0), StandardCharsets.UTF_8).trim();
                int nl = line.indexOf('\n');
                if (nl >= 0) {
                    line = line.substring(0, nl).trim();
                }
                if (!line.isEmpty()) {
                    return new File(line);
                }
            } catch (IOException e) {
                // fall through to the default
            }
        }
        return defaultDataDir();
    }

    private void saveChosen(File dir) {
        try {
            FileOutputStream out = new FileOutputStream(chosenFile());
            out.write((dir.getPath() + "\n").getBytes(StandardCharsets.UTF_8));
            out.close();
        } catch (IOException e) {
            status.setText("Nao consegui salvar a escolha: " + e);
        }
    }

    private static boolean hasEsm(File dir) {
        String[] names = dir.list();
        if (names == null) {
            return false;
        }
        for (String n : names) {
            if (n.equalsIgnoreCase("FalloutNV.esm")) {
                return true;
            }
        }
        return false;
    }

    /** If the player picked the game's root folder, the Data folder inside it is meant. */
    private static File normalize(File picked) {
        if (hasEsm(picked)) {
            return picked;
        }
        File inner = new File(picked, "Data");
        if (hasEsm(inner)) {
            return inner;
        }
        return picked;
    }

    private static boolean isClosedFolder(File dir) {
        String p = dir.getPath().replace('\\', '/');
        return p.contains("/Android/data/") || p.endsWith("/Android/data")
                || p.contains("/Android/obb/") || p.endsWith("/Android/obb");
    }

    private void pickFolder() {
        Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        startActivityForResult(i, REQUEST_TREE);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        if (requestCode == REQUEST_TREE && resultCode == RESULT_OK && data != null && data.getData() != null) {
            File path = treeToPath(data.getData());
            if (path != null) {
                saveChosen(normalize(path));
            }
        }
        refresh();
    }

    /** content://.../tree/primary%3AGames%2FFNV -> /storage/emulated/0/Games/FNV (other volumes: /storage/XXXX-XXXX/...). */
    private static File treeToPath(Uri uri) {
        String id = DocumentsContract.getTreeDocumentId(uri);
        int colon = id.indexOf(':');
        String volume = colon < 0 ? id : id.substring(0, colon);
        String rel = colon < 0 ? "" : id.substring(colon + 1);
        File base = "primary".equals(volume) ? Environment.getExternalStorageDirectory() : new File("/storage/" + volume);
        return rel.isEmpty() ? base : new File(base, rel);
    }

    private void createDefaultFolder() {
        if (!hasFilesAccess()) {
            status.setText("Conceda o acesso a todos os arquivos primeiro (botao 1).");
            return;
        }
        File dir = defaultDataDir();
        dir.mkdirs();
        saveChosen(dir);
        refresh();
    }

    // ---- state and start --------------------------------------------------------------

    private void refresh() {
        boolean access = hasFilesAccess();
        loadControls();
        controllerSwitch.setEnabled(access);
        touchSwitch.setEnabled(access);
        File data = dataDir();
        StringBuilder sb = new StringBuilder();
        sb.append("Acesso a todos os arquivos: ").append(access ? "concedido" : "NAO concedido").append("\n\n");
        sb.append("Pasta Data do jogo:\n").append(data.getPath()).append("\n\n");
        boolean ready = false;
        if (!access) {
            sb.append("Toque em 1 e ative a chave \"Permitir acesso para gerenciar todos os arquivos\".");
        } else if (isClosedFolder(data)) {
            sb.append("Essa pasta fica em Android/data ou Android/obb, que o Android bloqueia. Escolha outra (botao 2).");
        } else if (!data.isDirectory()) {
            sb.append("A pasta ainda nao existe. Escolha a Data do Fallout (botao 2) ou crie a padrao e copie o Data para la.");
        } else if (hasEsm(data)) {
            sb.append("FalloutNV.esm encontrado. Pode iniciar.");
            ready = true;
        } else {
            sb.append("FalloutNV.esm NAO esta nessa pasta. Escolha a pasta Data do Fallout (botao 2).");
        }
        sb.append("\n\nOpcional: arquivos nvforever-args.txt e nvforever-touch.txt na pasta NVForever do armazenamento do celular.");
        status.setText(sb.toString());
        permissionButton.setEnabled(!access);
        pickButton.setEnabled(access);
        defaultButton.setEnabled(access);
        startButton.setEnabled(access && !isClosedFolder(data));
        startButton.setText(ready ? "3. Iniciar o jogo" : "3. Iniciar mesmo assim (diagnostico)");
    }

    private void startGame() {
        if (!hasFilesAccess()) {
            refresh();
            return;
        }
        // The shared folder holds the option files; make sure it exists.
        new File(Environment.getExternalStorageDirectory(), "NVForever").mkdirs();
        if (!chosenFile().isFile()) {
            saveChosen(dataDir());
        }
        startActivity(new Intent(this, GameActivity.class).addFlags(Intent.FLAG_ACTIVITY_REORDER_TO_FRONT | Intent.FLAG_ACTIVITY_SINGLE_TOP));
    }
}

// Vectores de interoperabilidad con el verificador Java real.
//
// Usa las clases de Spring Security presentes en el repositorio Maven local:
// BCryptPasswordEncoder (formato legado del backend Java) y
// Argon2PasswordEncoder (formato PHC de destino).
//
// Sin dependencias de JSON: el protocolo es una línea por vector con campos
// separados por tabulador, de modo que el orquestador Node valida sin ambigüedad.
//
// Modos:
//   bcrypt         <  label \t password \t cost
//                  >  label \t $2a$hash
//   verify-argon2  <  label \t password \t phc
//                  >  label \t true|false
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;

import org.springframework.security.crypto.argon2.Argon2PasswordEncoder;
import org.springframework.security.crypto.bcrypt.BCryptPasswordEncoder;

public final class CredentialVectors {

    private CredentialVectors() {
    }

    public static void main(String[] args) throws Exception {
        if (args.length != 1) {
            System.err.println("uso: CredentialVectors <bcrypt|verify-argon2>");
            System.exit(2);
            return;
        }
        BufferedReader input = new BufferedReader(
                new InputStreamReader(System.in, StandardCharsets.UTF_8));
        PrintWriter output = new PrintWriter(
                new OutputStreamWriter(System.out, StandardCharsets.UTF_8), true);
        String line;
        while ((line = input.readLine()) != null) {
            String[] parts = line.split("\t", -1);
            if (parts.length != 3) {
                continue;
            }
            String label = parts[0];
            String password = parts[1];
            switch (args[0]) {
                case "bcrypt" -> {
                    int cost = Integer.parseInt(parts[2]);
                    BCryptPasswordEncoder encoder = new BCryptPasswordEncoder(cost);
                    output.println(label + "\t" + encoder.encode(password));
                }
                case "verify-argon2" -> {
                    // Parámetros equivalentes a lab-argon2-v1 para comparar formato.
                    Argon2PasswordEncoder encoder = new Argon2PasswordEncoder(16, 32, 1, 19456, 2);
                    boolean matches;
                    try {
                        matches = encoder.matches(password, parts[2]);
                    } catch (RuntimeException failure) {
                        matches = false;
                    }
                    output.println(label + "\t" + matches);
                }
                default -> {
                    System.err.println("modo desconocido");
                    System.exit(2);
                    return;
                }
            }
        }
    }
}

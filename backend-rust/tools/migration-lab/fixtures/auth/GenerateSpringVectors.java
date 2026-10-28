import java.nio.charset.StandardCharsets;
import org.springframework.security.crypto.bcrypt.BCrypt;

/** Generates only public synthetic test material, never application credentials. */
class GenerateSpringVectors {
    public static void main(String[] args) {
        String[] labels = {"ordinary", "ascii71", "ascii72", "utf8_71", "utf8_72"};
        String[] passwords = {"lab-only-Piñata-42!", "a".repeat(71), "a".repeat(72),
                "é".repeat(35) + "a", "é".repeat(36)};
        System.out.println("{\"synthetic_only\":true,\"producer\":\"Spring Security BCrypt 7.1.1\",\"cost\":4,\"vectors\":[");
        boolean first = true;
        for (String variant : new String[]{"2a", "2b", "2y"}) {
            for (int i = 0; i < passwords.length; i++) {
                String password = passwords[i];
                String encoded = BCrypt.hashpw(password, BCrypt.gensalt("$" + variant, 4));
                if (!BCrypt.checkpw(password, encoded) || BCrypt.checkpw("wrong-lab-only", encoded)) {
                    throw new IllegalStateException("Synthetic vector self-check failed");
                }
                if (!first) System.out.println(",");
                first = false;
                System.out.print("{\"label\":\"" + labels[i] + "\",\"variant\":\"" + variant
                        + "\",\"password\":\"" + password + "\",\"utf8_bytes\":"
                        + password.getBytes(StandardCharsets.UTF_8).length + ",\"encoded\":\"" + encoded + "\"}");
            }
        }
        System.out.println("\n]}");
    }
}

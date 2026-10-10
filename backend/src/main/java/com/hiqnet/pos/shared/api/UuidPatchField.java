package com.hiqnet.pos.shared.api;

import java.util.UUID;
import tools.jackson.core.JsonParser;
import tools.jackson.databind.DeserializationContext;
import tools.jackson.databind.ValueDeserializer;
import tools.jackson.databind.annotation.JsonDeserialize;
import tools.jackson.databind.exc.MismatchedInputException;

/**
 * UUID-typed companion of {@link PatchField}: explicit "field absent" vs
 * "field present, possibly null" for PATCH payloads carrying a UUID reference.
 * Needed where a plain {@code UUID} cannot distinguish {@code {}} (no change)
 * from {@code {"key": null}} (clear the reference):
 *
 * <pre>
 * {}                  -> field == null                (no change)
 * {"key": "&lt;uuid&gt;"}   -> field.value() == uuid       (set)
 * {"key": null}       -> field.value() == null        (clear/detach)
 * </pre>
 */
@JsonDeserialize(using = UuidPatchField.Deserializer.class)
public record UuidPatchField(UUID value, boolean present, boolean nullValue) {

    /** Present with an explicit value (possibly {@code null}). */
    public static UuidPatchField of(UUID value) {
        return new UuidPatchField(value, true, value == null);
    }

    static final class Deserializer extends ValueDeserializer<UuidPatchField> {

        @Override
        public UuidPatchField deserialize(JsonParser parser, DeserializationContext context) {
            String raw = parser.getValueAsString();
            if (raw == null) {
                throw MismatchedInputException.from(
                        parser, UUID.class, "Expected a UUID string but got a non-string token");
            }
            try {
                return UuidPatchField.of(UUID.fromString(raw));
            } catch (IllegalArgumentException ex) {
                throw MismatchedInputException.from(parser, UUID.class, "Expected a UUID string");
            }
        }

        @Override
        public UuidPatchField getNullValue(DeserializationContext context) {
            // Explicit "key": null -> present-null marker (clears the reference).
            return UuidPatchField.of(null);
        }

        @Override
        public UuidPatchField getAbsentValue(DeserializationContext context) {
            // Missing key -> null reference (omitted, no change).
            return null;
        }
    }
}

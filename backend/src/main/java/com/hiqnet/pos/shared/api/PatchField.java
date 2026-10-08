package com.hiqnet.pos.shared.api;

import tools.jackson.core.JsonParser;
import tools.jackson.databind.DeserializationContext;
import tools.jackson.databind.ValueDeserializer;
import tools.jackson.databind.annotation.JsonDeserialize;

/**
 * Explicit representation of "field absent" vs "field present, possibly null"
 * for PATCH payloads. A field of this type is {@code null} when the key is
 * missing from the JSON body; otherwise it holds a value that may itself be
 * null (explicit {@code "key": null}). Lets a PATCH distinguish:
 *
 * <pre>
 * {}                  -> field == null                (no change)
 * {"key": "v"}        -> field.value() == "v"        (set)
 * {"key": null}       -> field.value() == null       (clear/reset)
 * </pre>
 *
 * Currently used for the branch {@code timezone} override (null clears it and
 * falls back to inheriting the organization timezone).
 */
@JsonDeserialize(using = PatchField.Deserializer.class)
public record PatchField(String value, boolean present, boolean nullValue) {

    /** Present with an explicit value (possibly {@code null}). */
    public static PatchField of(String value) {
        return new PatchField(value, true, value == null);
    }

    /**
     * Jackson deserializer for the record. Missing key -> Jackson leaves the
     * property null (this deserializer is not invoked). JSON null -> Jackson
     * calls {@link #getNullValue} which returns the present-null marker.
     */
    static final class Deserializer extends ValueDeserializer<PatchField> {

        @Override
        public PatchField deserialize(JsonParser parser, DeserializationContext context) {
            return PatchField.of(parser.getValueAsString());
        }

        @Override
        public PatchField getNullValue(DeserializationContext context) {
            // Explicit "key": null -> present-null marker (clears the override).
            return PatchField.of(null);
        }

        @Override
        public PatchField getAbsentValue(DeserializationContext context) {
            // Missing key -> null reference (omitted, no change). Jackson's
            // default delegates absent to getNullValue, which would wrongly
            // treat {} as "clear"; override it explicitly.
            return null;
        }
    }
}
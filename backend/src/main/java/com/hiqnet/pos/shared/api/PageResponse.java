package com.hiqnet.pos.shared.api;

import java.util.List;
import java.util.function.Function;
import org.springframework.data.domain.Page;

/**
 * Stable pagination envelope for list endpoints. Spring Data's {@code Page} is
 * never serialized directly: the JSON contract ({@code items/page/size/
 * totalElements/totalPages}) is part of the public API and must not leak the
 * persistence framework's shape. Reused by future list endpoints.
 */
public record PageResponse<T>(List<T> items, int page, int size, long totalElements, int totalPages) {

    /** Maps a Spring Data page into the public envelope, applying a DTO mapper. */
    public static <E, T> PageResponse<T> from(Page<E> source, Function<E, T> mapper) {
        return new PageResponse<>(
                source.getContent().stream().map(mapper).toList(),
                source.getNumber(),
                source.getSize(),
                source.getTotalElements(),
                source.getTotalPages());
    }
}

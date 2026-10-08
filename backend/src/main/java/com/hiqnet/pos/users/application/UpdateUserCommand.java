package com.hiqnet.pos.users.application;

import com.hiqnet.pos.users.domain.UserStatus;

/** Partial update command: {@code null} means "not provided" (no change). */
public record UpdateUserCommand(long expectedVersion, String email, UserStatus status) {
}

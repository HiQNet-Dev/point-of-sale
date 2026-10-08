package com.hiqnet.pos.branch.api;

import com.hiqnet.pos.branch.application.BranchService;
import com.hiqnet.pos.branch.application.UpdateBranchCommand;
import jakarta.validation.Valid;
import java.util.List;
import java.util.UUID;
import org.springframework.http.HttpStatus;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PatchMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.ResponseStatus;
import org.springframework.web.bind.annotation.RestController;

@RestController
@RequestMapping("/api/v1/organizations/{organizationId}/branches")
public class BranchController {

    private final BranchService branchService;

    public BranchController(BranchService branchService) {
        this.branchService = branchService;
    }

    @PostMapping
    @ResponseStatus(HttpStatus.CREATED)
    public BranchResponse create(
            @PathVariable UUID organizationId, @Valid @RequestBody CreateBranchRequest request) {
        return BranchResponse.from(
                branchService.create(organizationId, request.name(), request.code(), request.timezone()));
    }

    @GetMapping
    public List<BranchResponse> list(@PathVariable UUID organizationId) {
        return branchService.list(organizationId).stream().map(BranchResponse::from).toList();
    }

    @GetMapping("/{branchId}")
    public BranchResponse get(@PathVariable UUID organizationId, @PathVariable UUID branchId) {
        return BranchResponse.from(branchService.get(organizationId, branchId));
    }

    @PatchMapping("/{branchId}")
    public BranchResponse update(
            @PathVariable UUID organizationId,
            @PathVariable UUID branchId,
            @Valid @RequestBody UpdateBranchRequest request) {
        return BranchResponse.from(branchService.update(
                organizationId, branchId, new UpdateBranchCommand(request.name(), request.timezone(), request.status())));
    }
}
package com.hiqnet.pos;

import org.springframework.boot.SpringApplication;

public class TestPosApplication {

	public static void main(String[] args) {
		SpringApplication.from(PosApplication::main).with(TestcontainersConfiguration.class).run(args);
	}

}

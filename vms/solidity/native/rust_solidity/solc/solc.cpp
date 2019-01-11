#include <stdio.h>
#include <string>
#include "CommandLineInterface.h"
#include <boost/exception/all.hpp>

using namespace std;

extern "C"
const char * solc_compile(char* sol) {
	dev::solidity::CommandLineInterface cli;

	char* argv[3];

	char bin[8];
	strncpy(bin, "solc", sizeof(bin));
	argv[0] = bin;

	char option1[32];
	strncpy(option1, "--combined-json=abi,bin,opcodes", sizeof(option1));
	argv[1] = option1;

	char option2[15];
	strcpy(option2, "--static-call");
	argv[2] = option2;

	cli.setSourceCodes(sol);

	static string results = "";

	if (!cli.parseArguments(3, argv))
		return results.append("Failed to parse arguments"). c_str();
	if (!cli.processInput()) {
		results = cli.getResults();
		return results.append("Failed to process input"). c_str();
	}
	try
	{
		cli.actOnInput();
		results = cli.getResults();
	}
	catch (boost::exception const& _exception)
	{
		cerr << "Exception during output generation: " << boost::diagnostic_information(_exception) << endl;
	}

	return results.c_str();
}
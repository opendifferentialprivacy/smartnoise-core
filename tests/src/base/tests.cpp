#include <catch2/catch.hpp>
#include <differential_privacy/graph.hpp>
#include "analysis.pb.h"

#include <iostream>
#include "../../include/tests/main.hpp"

TEST_CASE("Validate_1", "[Validate]") {
    Analysis* analysis = make_test_analysis();

    std::string message = analysis->SerializeAsString();
    std::cout << analysis->DebugString();

    assert(validateAnalysis(&message[0], message.length()));
}

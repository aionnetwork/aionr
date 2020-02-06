pragma solidity ^0.4.15;

contract Simple {
    function taker(uint _a, uint _b) {
        // do something with _a and _b.
    }
}

contract Simple2 {
    function arithmetics(uint _a, uint _b) returns (uint o_sum, uint o_product) {
        o_sum = _a + _b;
        o_product = _a * _b;
    }
}

contract C {
    function g(uint a) returns (uint ret) { return f(); }
    function f() returns (uint ret) { return g(7) + f(); }
}

contract InfoFeed {
    function info() payable returns (uint ret) { return 42; }
}

contract Consumer {
    InfoFeed feed;
    function setFeed(address addr) { feed = InfoFeed(addr); }
    function callFeed() { feed.info.value(10).gas(800)(); }
}

contract C2 {
    function f(uint key, uint value) {
        // ...
    }

    function g() {
        // named arguments
        f({value: 2, key: 3});
    }
}

contract C3 {
    // omitted name for parameter
    function func(uint k, uint) returns(uint) {
        return k;
    }
}

contract D {
    uint x;
    function D(uint a) payable {
        x = a;
    }
}

contract C4 {
    D d = new D(4); // will be executed as part of C's constructor

    function createD(uint arg) {
        D newD = new D(arg);
    }

    function createAndEndowD(uint arg, uint amount) {
        // Send ether along with the creation
        D newD = (new D).value(amount)(arg);
    }
}

contract C5 {
    uint[] data;

    function f() returns (uint, bool, uint) {
        return (7, true, 2);
    }

    function g() {
        // Declares and assigns the variables. Specifying the type explicitly is not possible.
        var (x, b, y) = f();
        // Assigns to a pre-existing variable.
        (x, y) = (2, 7);
        // Common trick to swap values -- does not work for non-value storage types.
        (x, y) = (y, x);
        // Components can be left out (also for variable declarations).
        // If the tuple ends in an empty component,
        // the rest of the values are discarded.
        (data.length,) = f(); // Sets the length to 7
        // The same can be done on the left side.
        (,data[3]) = f(); // Sets data[3] to 2
        // Components can only be left out at the left-hand-side of assignments, with
        // one exception:
        (x,) = (1,);
        // (1,) is the only way to specify a 1-component tuple, because (1) is
        // equivalent to 1.
    }
}

contract ScopingErrors {
    function scoping() {
        uint i = 0;

        while (i++ < 1) {
            uint same1 = 0;
        }

        while (i++ < 2) {
            uint same11 = 0;// Illegal, second declaration of same1
        }
    }

    function minimalScoping() {
        {
            uint same2 = 0;
        }

        {
            uint same22 = 0;// Illegal, second declaration of same2
        }
    }

    function forLoopScoping() {
        for (uint same3 = 0; same3 < 1; same3++) {
        }

        for (uint same33 = 0; same33 < 1; same33++) {// Illegal, second declaration of same3
        }
    }
}

contract C6 {
	function foo() returns (uint) {
	    // baz is implicitly initialized as 0
	    uint bar = 5;
	    if (true) {
	        bar += baz;
	    } else {
	        uint baz = 10;// never executes
	    }
	    return bar;// returns 5
	}
}

contract Sharer {
    function sendHalf(address addr) payable returns (uint balance) {
        require(msg.value % 2 == 0); // Only allow even numbers
        uint balanceBeforeTransfer = this.balance;
        addr.transfer(msg.value / 2);
        // Since transfer throws an exception on failure and
        // cannot call back here, there should be no way for us to
        // still have half of the money.
        assert(this.balance == balanceBeforeTransfer - msg.value / 2);
        return this.balance;
    }
}


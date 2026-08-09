import Foundation

private final class Completion<Value>: @unchecked Sendable {
    private let condition = NSCondition()
    private var result: Result<Value, Error>?

    func complete(_ value: Result<Value, Error>) {
        condition.lock()
        result = value
        condition.broadcast()
        condition.unlock()
    }

    func wait() throws -> Value {
        condition.lock()
        while result == nil { condition.wait() }
        let value = result!
        condition.unlock()
        return try value.get()
    }
}

private final class FifoState: @unchecked Sendable {
    let condition = NSCondition()
    var commands: [@Sendable () -> Void] = []
    var owner: Thread?
    var stopping = false
    var stopped = false

    func run() {
        condition.lock()
        owner = Thread.current
        condition.broadcast()
        while true {
            while commands.isEmpty && !stopping { condition.wait() }
            if commands.isEmpty && stopping { break }
            let command = commands.removeFirst()
            condition.unlock()
            command()
            condition.lock()
        }
        stopped = true
        condition.broadcast()
        condition.unlock()
    }
}

final class StableFifoThread: @unchecked Sendable {
    private let state: FifoState
    private let thread: Thread

    init(name: String) {
        let state = FifoState()
        self.state = state
        thread = Thread { state.run() }
        thread.name = name
        thread.qualityOfService = .userInitiated
        thread.start()
        state.condition.lock()
        while state.owner == nil { state.condition.wait() }
        state.condition.unlock()
    }

    func call<Value>(_ body: @escaping @Sendable () throws -> Value) throws -> Value {
        state.condition.lock()
        let onOwner = state.owner === Thread.current
        state.condition.unlock()
        if onOwner { return try body() }
        let completion = Completion<Value>()
        try enqueue {
            do { completion.complete(.success(try body())) }
            catch { completion.complete(.failure(error)) }
        }
        return try completion.wait()
    }

    func enqueue(_ body: @escaping @Sendable () -> Void) throws {
        state.condition.lock()
        if state.stopping {
            state.condition.unlock()
            throw ContractFailure(description: "stable FIFO is closed")
        }
        state.commands.append(body)
        state.condition.signal()
        state.condition.unlock()
    }

    func barrier() throws { try call {} }

    func shutdown() throws {
        state.condition.lock()
        if state.owner === Thread.current {
            state.condition.unlock()
            throw ContractFailure(description: "stable FIFO cannot join itself")
        }
        state.stopping = true
        state.condition.signal()
        while !state.stopped { state.condition.wait() }
        state.condition.unlock()
    }
}

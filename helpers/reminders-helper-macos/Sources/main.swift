import Foundation
import EventKit

// MARK: - Data Models

struct Reminder: Codable {
    let id: String
    let title: String
    let list: String
    let completed: Bool
    let dueDate: String?
    let priority: Int
    let notes: String?
    let creationDate: String?
}

struct Request: Codable {
    let action: String
    let list: String?
    let completed: Bool?
    let id: String?
    let title: String?
    let dueDate: String?
    let notes: String?
    let priority: Int?
}

struct Response: Codable {
    let reminders: [Reminder]?
    let error: String?
    let success: Bool?
}

struct ErrorResponse: Codable {
    let error: String
}

// MARK: - RemindersHelper

class RemindersHelper {
    let eventStore = EKEventStore()
    let dateFormatter = ISO8601DateFormatter()
    var accessGranted = false
    
    init() {
        dateFormatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    }
    
    // Request access to reminders
    func requestAccess() async throws {
        if #available(macOS 14.0, *) {
            let granted = try await eventStore.requestFullAccessToReminders()
            accessGranted = granted
        } else {
            let granted = try await eventStore.requestAccess(to: .reminder)
            accessGranted = granted
        }
        
        if !accessGranted {
            throw NSError(domain: "RemindersHelper", code: 1, userInfo: [
                NSLocalizedDescriptionKey: "Access to reminders denied"
            ])
        }
    }
    
    // Convert EKReminder to Reminder
    func convertReminder(_ ekReminder: EKReminder) -> Reminder {
        let listName = ekReminder.calendar?.title ?? "Unknown"
        let dueDate = ekReminder.dueDateComponents?.date.map { dateFormatter.string(from: $0) }
        let creationDate = ekReminder.creationDate.map { dateFormatter.string(from: $0) }
        
        return Reminder(
            id: ekReminder.calendarItemIdentifier,
            title: ekReminder.title ?? "",
            list: listName,
            completed: ekReminder.isCompleted,
            dueDate: dueDate,
            priority: ekReminder.priority,
            notes: ekReminder.notes,
            creationDate: creationDate
        )
    }
    
    // List reminders
    func listReminders(listName: String?, completed: Bool?) async throws -> [Reminder] {
        if !accessGranted {
            try await requestAccess()
        }
        
        let calendars = eventStore.calendars(for: .reminder)
        
        // Filter by list name if specified
        let targetCalendars: [EKCalendar]
        if let listName = listName {
            targetCalendars = calendars.filter { $0.title == listName }
            if targetCalendars.isEmpty {
                throw NSError(domain: "RemindersHelper", code: 2, userInfo: [
                    NSLocalizedDescriptionKey: "List '\(listName)' not found"
                ])
            }
        } else {
            targetCalendars = calendars
        }
        
        // Create predicate
        let predicate = eventStore.predicateForReminders(in: targetCalendars)
        
        // Fetch reminders (using completion handler, not async/await for EventKit)
        let ekReminders = try await withCheckedThrowingContinuation { continuation in
            eventStore.fetchReminders(matching: predicate) { reminders in
                if let reminders = reminders {
                    continuation.resume(returning: reminders)
                } else {
                    continuation.resume(returning: [])
                }
            }
        }
        
        // Filter by completion status if specified
        let filteredReminders: [EKReminder]
        if let completed = completed {
            filteredReminders = ekReminders.filter { $0.isCompleted == completed }
        } else {
            filteredReminders = ekReminders
        }
        
        return filteredReminders.map { convertReminder($0) }
    }
    
    // Add reminder
    func addReminder(listName: String, title: String, dueDate: String?, notes: String?, priority: Int?) async throws -> Reminder {
        if !accessGranted {
            try await requestAccess()
        }
        
        // Find the calendar/list
        let calendars = eventStore.calendars(for: .reminder)
        guard let calendar = calendars.first(where: { $0.title == listName }) else {
            throw NSError(domain: "RemindersHelper", code: 2, userInfo: [
                NSLocalizedDescriptionKey: "List '\(listName)' not found"
            ])
        }
        
        // Create reminder
        let reminder = EKReminder(eventStore: eventStore)
        reminder.calendar = calendar
        reminder.title = title
        
        // Set due date if provided
        if let dueDateStr = dueDate, let date = dateFormatter.date(from: dueDateStr) {
            let components = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: date)
            reminder.dueDateComponents = components
        }
        
        // Set notes
        if let notes = notes {
            reminder.notes = notes
        }
        
        // Set priority
        if let priority = priority {
            reminder.priority = priority
        }
        
        // Save
        try eventStore.save(reminder, commit: true)
        
        return convertReminder(reminder)
    }
    
    // Update reminder
    func updateReminder(id: String, title: String?, dueDate: String?, notes: String?, priority: Int?) async throws -> Reminder {
        if !accessGranted {
            try await requestAccess()
        }
        
        // Find the reminder
        guard let reminder = eventStore.calendarItem(withIdentifier: id) as? EKReminder else {
            throw NSError(domain: "RemindersHelper", code: 3, userInfo: [
                NSLocalizedDescriptionKey: "Reminder with id '\(id)' not found"
            ])
        }
        
        // Update fields
        if let title = title {
            reminder.title = title
        }
        
        if let dueDateStr = dueDate, let date = dateFormatter.date(from: dueDateStr) {
            let components = Calendar.current.dateComponents([.year, .month, .day, .hour, .minute], from: date)
            reminder.dueDateComponents = components
        }
        
        if let notes = notes {
            reminder.notes = notes
        }
        
        if let priority = priority {
            reminder.priority = priority
        }
        
        // Save
        try eventStore.save(reminder, commit: true)
        
        return convertReminder(reminder)
    }
    
    // Complete reminder
    func completeReminder(id: String) async throws -> Reminder {
        if !accessGranted {
            try await requestAccess()
        }
        
        // Find the reminder
        guard let reminder = eventStore.calendarItem(withIdentifier: id) as? EKReminder else {
            throw NSError(domain: "RemindersHelper", code: 3, userInfo: [
                NSLocalizedDescriptionKey: "Reminder with id '\(id)' not found"
            ])
        }
        
        // Mark as completed
        reminder.isCompleted = true
        reminder.completionDate = Date()
        
        // Save
        try eventStore.save(reminder, commit: true)
        
        return convertReminder(reminder)
    }
    
    // Delete reminder
    func deleteReminder(id: String) async throws {
        if !accessGranted {
            try await requestAccess()
        }
        
        // Find the reminder
        guard let reminder = eventStore.calendarItem(withIdentifier: id) as? EKReminder else {
            throw NSError(domain: "RemindersHelper", code: 3, userInfo: [
                NSLocalizedDescriptionKey: "Reminder with id '\(id)' not found"
            ])
        }
        
        // Delete
        try eventStore.remove(reminder, commit: true)
    }
    
    // Process request
    func processRequest(_ request: Request) async throws -> Response {
        switch request.action {
        case "list":
            let reminders = try await listReminders(listName: request.list, completed: request.completed)
            return Response(reminders: reminders, error: nil, success: true)
            
        case "add":
            guard let title = request.title, let list = request.list else {
                throw NSError(domain: "RemindersHelper", code: 4, userInfo: [
                    NSLocalizedDescriptionKey: "Missing required parameters: title and list"
                ])
            }
            let reminder = try await addReminder(
                listName: list,
                title: title,
                dueDate: request.dueDate,
                notes: request.notes,
                priority: request.priority
            )
            return Response(reminders: [reminder], error: nil, success: true)
            
        case "update":
            guard let id = request.id else {
                throw NSError(domain: "RemindersHelper", code: 4, userInfo: [
                    NSLocalizedDescriptionKey: "Missing required parameter: id"
                ])
            }
            let reminder = try await updateReminder(
                id: id,
                title: request.title,
                dueDate: request.dueDate,
                notes: request.notes,
                priority: request.priority
            )
            return Response(reminders: [reminder], error: nil, success: true)
            
        case "complete":
            guard let id = request.id else {
                throw NSError(domain: "RemindersHelper", code: 4, userInfo: [
                    NSLocalizedDescriptionKey: "Missing required parameter: id"
                ])
            }
            let reminder = try await completeReminder(id: id)
            return Response(reminders: [reminder], error: nil, success: true)
            
        case "delete":
            guard let id = request.id else {
                throw NSError(domain: "RemindersHelper", code: 4, userInfo: [
                    NSLocalizedDescriptionKey: "Missing required parameter: id"
                ])
            }
            try await deleteReminder(id: id)
            return Response(reminders: nil, error: nil, success: true)
            
        default:
            throw NSError(domain: "RemindersHelper", code: 5, userInfo: [
                NSLocalizedDescriptionKey: "Unknown action: \(request.action)"
            ])
        }
    }
}

// MARK: - Main

func runHelper() async {
    let helper = RemindersHelper()
    let decoder = JSONDecoder()
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys]
    
    // Read from stdin
    guard let line = readLine() else {
        let error = ErrorResponse(error: "No input provided")
        if let json = try? encoder.encode(error), let str = String(data: json, encoding: .utf8) {
            print(str)
        }
        exit(1)
    }
    
    // Parse request
    guard let data = line.data(using: .utf8) else {
        let error = ErrorResponse(error: "Invalid UTF-8 input")
        if let json = try? encoder.encode(error), let str = String(data: json, encoding: .utf8) {
            print(str)
        }
        exit(1)
    }
    
    do {
        let request = try decoder.decode(Request.self, from: data)
        let response = try await helper.processRequest(request)
        let json = try encoder.encode(response)
        if let str = String(data: json, encoding: .utf8) {
            print(str)
        }
    } catch {
        let errorResponse = ErrorResponse(error: error.localizedDescription)
        if let json = try? encoder.encode(errorResponse), let str = String(data: json, encoding: .utf8) {
            print(str)
        }
        exit(1)
    }
}

// Entry point
Task {
    await runHelper()
    exit(0)
}

// Keep the program running
RunLoop.main.run()

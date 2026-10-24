// Every place in the game's own code where a pause can come from.
//
//     pause_sites <sts2.dll>
//
// A choice pauses where the code awaits `PlayerChoiceContext.SignalPlayerChoiceBegun`.
// A method can pause where it calls one that can, so this walks the call graph
// back from that method. A virtual call can reach every override of the method
// it names, so a call to `AbstractModel.AfterDeath` pauses if any model's
// override can. An async method's body lives in its state machine, and a
// lambda's in a method of a closure class, so both count as their owner's.
//
// It prints one line per call that can pause, inside a method of the game's
// content or commands, in the order the calls stand in the method:
//
//     <owner type> <owner method> -> <callee type>.<callee method>
//
// The simulator needs a resume point at each one.

using Mono.Cecil;
using Mono.Cecil.Cil;

if (args.Length is not (1 or 3) || (args.Length == 3 && args[1] != "--why"))
{
    Console.Error.WriteLine("usage: pause_sites <sts2.dll> [--why <Type.Method>]");
    return 2;
}

var module = ModuleDefinition.ReadModule(args[0]);
var types = module.GetTypes().ToList();
var methods = types.SelectMany(type => type.Methods).ToList();

// The state machine type of each async or iterator method, to its owner.
var machines = new Dictionary<TypeDefinition, MethodDefinition>();
foreach (var method in methods)
{
    foreach (var attribute in method.CustomAttributes)
    {
        var name = attribute.AttributeType.Name;
        if (name is not ("AsyncStateMachineAttribute" or "IteratorStateMachineAttribute"
            or "AsyncIteratorStateMachineAttribute"))
        {
            continue;
        }
        if (attribute.ConstructorArguments[0].Value is TypeReference machine
            && machine.Resolve() is { } resolved)
        {
            machines[resolved] = method;
        }
    }
}

// The method a lambda or local function was written in: `<Owner>b__3_0` or
// `<Owner>g__Local|3_0`, in the owner's type or a closure class nested in it.
MethodDefinition? Written(MethodDefinition method)
{
    if (!method.Name.StartsWith('<'))
    {
        return null;
    }
    var close = method.Name.IndexOf('>');
    if (close <= 1)
    {
        return null;
    }
    var owner = method.Name[1..close];
    var type = method.DeclaringType;
    while (type is not null)
    {
        var found = type.Methods.FirstOrDefault(candidate => candidate.Name == owner);
        if (found is not null)
        {
            return found;
        }
        type = type.DeclaringType;
    }
    return null;
}

var owners = new Dictionary<MethodDefinition, MethodDefinition>();
MethodDefinition Owner(MethodDefinition method)
{
    if (owners.TryGetValue(method, out var known))
    {
        return known;
    }
    var owner = method;
    for (var guard = 0; guard < 16; guard++)
    {
        if (machines.TryGetValue(owner.DeclaringType, out var machineOwner))
        {
            owner = machineOwner;
            continue;
        }
        if (Written(owner) is { } written && written != owner)
        {
            owner = written;
            continue;
        }
        break;
    }
    owners[method] = owner;
    return owner;
}

// Each virtual method to the methods that override it, down every chain.
string Signature(MethodReference method) =>
    method.Name + "(" + string.Join(",", method.Parameters.Select(p => p.ParameterType.FullName)) + ")";

var overrides = new Dictionary<MethodDefinition, List<MethodDefinition>>();
void Overrides(MethodDefinition overridden, MethodDefinition by)
{
    if (!overrides.TryGetValue(overridden, out var list))
    {
        overrides[overridden] = list = new List<MethodDefinition>();
    }
    list.Add(by);
}
TypeDefinition? Resolve(TypeReference? type)
{
    try
    {
        return type?.Resolve();
    }
    catch (AssemblyResolutionException)
    {
        return null;
    }
}
foreach (var method in methods.Where(method => method.IsVirtual))
{
    foreach (var explicitly in method.Overrides)
    {
        if (explicitly.Resolve() is { } resolved)
        {
            Overrides(resolved, method);
        }
    }
    var signature = Signature(method);
    for (var type = Resolve(method.DeclaringType.BaseType); type is not null; type = Resolve(type.BaseType))
    {
        if (type.Module != module)
        {
            break;
        }
        var overridden = type.Methods.FirstOrDefault(
            candidate => candidate.IsVirtual && Signature(candidate) == signature);
        if (overridden is not null)
        {
            Overrides(overridden, method);
        }
    }
    foreach (var implemented in method.DeclaringType.Interfaces)
    {
        var face = Resolve(implemented.InterfaceType);
        var declared = face?.Methods.FirstOrDefault(candidate => Signature(candidate) == signature);
        if (declared is not null && face!.Module == module)
        {
            Overrides(declared, method);
        }
    }
}

IEnumerable<MethodDefinition> Targets(MethodDefinition callee, bool dispatched)
{
    yield return callee;
    if (!dispatched || !callee.IsVirtual)
    {
        yield break;
    }
    var seen = new HashSet<MethodDefinition> { callee };
    var stack = new Stack<MethodDefinition>();
    stack.Push(callee);
    while (stack.Count > 0)
    {
        if (!overrides.TryGetValue(stack.Pop(), out var list))
        {
            continue;
        }
        foreach (var by in list.Where(seen.Add))
        {
            yield return by;
            stack.Push(by);
        }
    }
}

bool IsSignal(MethodReference method) => method.Name == "SignalPlayerChoiceBegun";

// A pause boundary: `AssignTaskAndWaitForPauseOrCompletion` and its sibling.
// A listener a method calls through one pauses alone, and the method runs on,
// so a pause behind a dispatched call in such a method stops there.
bool IsBoundary(MethodReference method) =>
    method.Name is "AssignTaskAndWaitForPauseOrCompletion" or "WaitForPauseOrCompletionWithoutAssigningTask";

// The calls each owner makes, in order: the method named, and each method
// it can land in.
var calls = new Dictionary<MethodDefinition, List<(MethodReference Named, MethodDefinition? Itself, List<MethodDefinition> Lands, bool Dispatched)>>();
foreach (var method in methods.Where(method => method.HasBody))
{
    var owner = Owner(method);
    if (!calls.TryGetValue(owner, out var list))
    {
        calls[owner] = list = new();
    }
    foreach (var instruction in method.Body.Instructions)
    {
        var dispatched = instruction.OpCode.Code == Code.Callvirt;
        if (instruction.OpCode.Code is not (Code.Call or Code.Callvirt or Code.Newobj or Code.Ldftn or Code.Ldvirtftn)
            || instruction.Operand is not MethodReference named)
        {
            continue;
        }
        MethodDefinition? resolved;
        try
        {
            resolved = named.Resolve();
        }
        catch (AssemblyResolutionException)
        {
            resolved = null;
        }
        var lands = resolved is null || resolved.Module != module
            ? new List<MethodDefinition>()
            : Targets(resolved, dispatched || instruction.OpCode.Code == Code.Ldvirtftn)
                .Select(Owner)
                .Where(target => target != owner)
                .Distinct()
                .ToList();
        var itself = resolved is null || resolved.Module != module ? null : Owner(resolved);
        list.Add((named, itself == owner ? null : itself, lands, dispatched));
    }
}

// Back from the signal to every owner that pauses by itself: on a choice of
// its own, or through a call to a method that does. A virtual call is followed
// only to the method it names. Where one of its overrides pauses, the call
// pauses through dispatch, which the second list below names.
HashSet<MethodDefinition> Pausing()
{
    var pausing = new HashSet<MethodDefinition>();
    var callers = new Dictionary<MethodDefinition, List<MethodDefinition>>();
    foreach (var (owner, list) in calls)
    {
        foreach (var (named, itself, _, _) in list)
        {
            if (IsSignal(named))
            {
                pausing.Add(owner);
            }
            if (itself is null)
            {
                continue;
            }
            if (!callers.TryGetValue(itself, out var from))
            {
                callers[itself] = from = new();
            }
            from.Add(owner);
        }
    }
    var queue = new Queue<MethodDefinition>(pausing);
    while (queue.Count > 0)
    {
        var method = queue.Dequeue();
        if (!callers.TryGetValue(method, out var from))
        {
            continue;
        }
        foreach (var caller in from.Where(pausing.Add))
        {
            queue.Enqueue(caller);
        }
    }
    return pausing;
}
var pausing = Pausing();

string Name(MethodDefinition method) => $"{method.DeclaringType.FullName.Replace("MegaCrit.Sts2.Core.", "")}.{method.Name}";

// `--why`: the shortest chain of calls from a method to a signal, following
// each virtual call into the overrides it can land in.
if (args.Length == 3)
{
    // A virtual method starts from every override too.
    var starts = calls.Keys
        .Where(method => Name(method) == args[2])
        .SelectMany(method => Targets(method, true))
        .Select(Owner)
        .Distinct()
        .ToList();
    if (starts.Count == 0)
    {
        Console.Error.WriteLine($"no method {args[2]}");
        return 1;
    }
    var from = new Dictionary<MethodDefinition, (MethodDefinition Caller, string Call)>();
    var bounded = calls
        .Where(entry => entry.Value.Any(call => IsBoundary(call.Named)))
        .Select(entry => entry.Key)
        .ToHashSet();
    var frontier = new Queue<MethodDefinition>(starts);
    var seen = new HashSet<MethodDefinition>(starts);
    while (frontier.Count > 0)
    {
        var method = frontier.Dequeue();
        foreach (var (named, _, lands, dispatched) in calls.GetValueOrDefault(method, new()))
        {
            if (dispatched && bounded.Contains(method) && !starts.Contains(method))
            {
                continue;
            }
            if (IsSignal(named))
            {
                var chain = new List<string> { $"{Name(method)} -> {named.DeclaringType.Name}.{named.Name}" };
                for (var at = method; from.TryGetValue(at, out var step); at = step.Caller)
                {
                    chain.Add($"{Name(step.Caller)} -> {step.Call}");
                }
                chain.Reverse();
                foreach (var line in chain)
                {
                    Console.WriteLine(line);
                }
                return 0;
            }
            foreach (var target in lands.Where(seen.Add))
            {
                from[target] = (method, Name(target));
                frontier.Enqueue(target);
            }
        }
    }
    Console.WriteLine($"{args[2]} cannot pause");
    return 0;
}

// The content and the commands the simulator ports, less the test content.
string Space(TypeDefinition type)
{
    var space = type.Namespace;
    for (var outer = type; string.IsNullOrEmpty(space) && outer.DeclaringType is not null; outer = outer.DeclaringType)
    {
        space = outer.DeclaringType.Namespace;
    }
    return space;
}
bool Ported(TypeDefinition type)
{
    var space = Space(type);
    return (space.StartsWith("MegaCrit.Sts2.Core.Models.") || space == "MegaCrit.Sts2.Core.Commands")
        && !space.EndsWith(".Mocks");
}
string Short(TypeDefinition type) => type.FullName.Replace("MegaCrit.Sts2.Core.", "");

// Each owner that pauses by itself, and each call in it that does.
var sites = new List<string>();
foreach (var (owner, list) in calls)
{
    if (!pausing.Contains(owner) || !Ported(owner.DeclaringType) || owner.Name.StartsWith('<'))
    {
        continue;
    }
    foreach (var (named, itself, _, _) in list)
    {
        if (IsSignal(named))
        {
            sites.Add($"pauses {Short(owner.DeclaringType)}.{owner.Name} -> {named.DeclaringType.Name}.{named.Name}");
        }
        else if (itself is not null && pausing.Contains(itself))
        {
            sites.Add($"pauses {Short(owner.DeclaringType)}.{owner.Name} -> {Short(itself.DeclaringType)}.{itself.Name}");
        }
    }
}

// Each overridable method with an override that pauses by itself: a call to
// it, such as a hook raised on every listener, pauses through dispatch.
var throughOverrides = new List<string>();
foreach (var (overridden, _) in overrides)
{
    if (overrides.Keys.Any(other => overrides[other].Contains(overridden)))
    {
        continue;
    }
    var by = Targets(overridden, true)
        .Where(method => method != overridden && pausing.Contains(method) && Ported(method.DeclaringType))
        .Select(method => Short(method.DeclaringType))
        .Distinct()
        .OrderBy(name => name, StringComparer.Ordinal)
        .ToList();
    if (by.Count > 0)
    {
        throughOverrides.Add($"dispatches {Short(overridden.DeclaringType)}.{overridden.Name} -> {string.Join(" ", by)}");
    }
}

sites.Sort(StringComparer.Ordinal);
throughOverrides.Sort(StringComparer.Ordinal);
foreach (var line in sites.Concat(throughOverrides))
{
    Console.WriteLine(line);
}
Console.Error.WriteLine($"{sites.Count} calls that pause by themselves, {throughOverrides.Count} methods that pause through an override");
return 0;

# pyre-strict
from __future__ import absolute_import, division, print_function, unicode_literals

import copy
import difflib
import inspect
import itertools
import lib2to3.patcomp  # pyre-ignore: Pyre can't find this
import lib2to3.pgen2
import lib2to3.pygram
import lib2to3.pytree as pytree
import operator
import os.path
import pprint
import textwrap
from typing import AbstractSet, Iterable, Mapping, Optional, Sequence, Tuple, Union

from lspcommand import LspCommandProcessor, Transcript, TranscriptEntry
from utils import Json, interpolate_variables


_MessageSpec = Union[
    "_RequestSpec",
    "_NotificationSpec",
    "_WaitForNotificationSpec",
    "_WaitForRequestSpec",
]


_LspIdMap = Mapping[_MessageSpec, Json]

_Traceback = Sequence[inspect.FrameInfo]


class LspTestSpec:
    """Represents an LSP test to be run, in a declarative fashion.

    Since `LspTestSpec`s are just values, they can be composed using regular
    functions. For example, you can make an `initialize_spec` function that
    returns an `LspTestSpec` with the `initialize` request already sent and
    checked."""

    def __init__(self, name: str) -> None:
        self.name = name
        self._messages: Sequence["_MessageSpec"] = []
        self._ignored_notification_methods: AbstractSet[str] = set()

    def ignore_notifications(self, *, method: str) -> "LspTestSpec":
        ignored_notification_methods = set(self._ignored_notification_methods)
        ignored_notification_methods.add(method)
        return self._update(ignored_notification_methods=ignored_notification_methods)

    def request(
        self,
        method: str,
        params: Json,
        *,
        result: Json,
        wait: bool = True,
        comment: Optional[str] = None,
        powered_by: Optional[str] = None,
    ) -> "LspTestSpec":
        traceback = inspect.stack()
        assert traceback is not None, "Failed to get traceback info"

        messages = list(self._messages)
        messages.append(
            _RequestSpec(
                method=method,
                params=params,
                result=result,
                wait=wait,
                comment=comment,
                powered_by=powered_by,
                traceback=traceback,
            )
        )
        return self._update(messages=messages)

    def notification(
        self, method: str, params: Json, *, comment: Optional[str] = None
    ) -> "LspTestSpec":
        messages = list(self._messages)
        messages.append(
            _NotificationSpec(method=method, params=params, comment=comment)
        )
        return self._update(messages=messages)

    def wait_for_server_request(
        self, method: str, params: Json, *, result: Json, comment: Optional[str] = None
    ) -> "LspTestSpec":
        messages = list(self._messages)
        messages.append(
            _WaitForRequestSpec(
                method=method, params=params, result=result, comment=comment
            )
        )
        return self._update(messages=messages)

    def wait_for_notification(
        self, method: str, params: Json, *, comment: Optional[str] = None
    ) -> "LspTestSpec":
        messages = list(self._messages)
        messages.append(
            _WaitForNotificationSpec(method=method, params=params, comment=comment)
        )
        return self._update(messages=messages)

    def run(
        self, lsp_command_processor: LspCommandProcessor, variables: Mapping[str, str]
    ) -> Tuple[Transcript, Optional[str]]:
        """Run the test given the LSP command processor.

        Raises an exception with useful debugging information if the test fails."""
        (json_commands, lsp_id_map) = self._get_json_commands(variables=variables)
        transcript = lsp_command_processor.communicate(json_commands=json_commands)
        errors = list(
            self._verify_transcript(transcript=transcript, lsp_id_map=lsp_id_map)
        )
        if errors:
            num_errors = len(errors)
            error_details = (
                f"Test case {self.name} failed with {num_errors} errors:\n\n"
            )
            for i, error in enumerate(errors, 1):
                error_details += f"Error {i}/{num_errors}:\n"
                error_details += str(error) + "\n"
            error_details += """\
If you want to examine the raw LSP logs, you can check the `.sent.log` and
`.received.log` files that were generated in the template repo for this test."""
        else:
            error_details = None
        return (transcript, error_details)

    ### Internal. ###

    def _update(
        self,
        messages: Optional[Sequence["_MessageSpec"]] = None,
        ignored_notification_methods: Optional[AbstractSet[str]] = None,
    ) -> "LspTestSpec":
        spec = copy.copy(self)
        if messages is not None:
            spec._messages = messages
        if ignored_notification_methods is not None:
            spec._ignored_notification_methods = ignored_notification_methods
        return spec

    def _get_json_commands(
        self, variables: Mapping[str, str]
    ) -> Tuple[Sequence[Json], "_LspIdMap"]:
        """Transforms this test spec into something the LSP command processor
        can interpret."""
        json_commands = []
        lsp_id_map = {}
        current_id = 0
        for message in self._messages:
            current_id += 1
            lsp_id_map[message] = current_id

            if isinstance(message, _RequestSpec):
                json_commands.append(
                    {
                        "jsonrpc": "2.0",
                        "comment": message.comment,
                        "id": current_id,
                        "method": message.method,
                        "params": interpolate_variables(
                            message.params, variables=variables
                        ),
                    }
                )

                if message.wait:
                    json_commands.append(
                        {
                            "jsonrpc": "2.0",
                            "method": "$test/waitForResponse",
                            "params": {"id": current_id},
                        }
                    )
            elif isinstance(message, _NotificationSpec):
                json_commands.append(
                    {
                        "jsonrpc": "2.0",
                        "comment": message.comment,
                        "method": message.method,
                        "params": interpolate_variables(
                            message.params, variables=variables
                        ),
                    }
                )
            elif isinstance(message, _WaitForRequestSpec):
                json_commands.append(
                    {
                        "jsonrpc": "2.0",
                        "comment": message.comment,
                        "method": "$test/waitForRequest",
                        "params": {
                            "method": message.method,
                            "params": message.params,
                            "result": message.result,
                        },
                    }
                )
            elif isinstance(message, _WaitForNotificationSpec):
                json_commands.append(
                    {
                        "jsonrpc": "2.0",
                        "comment": message.comment,
                        "method": "$test/waitForNotification",
                        "params": {"method": message.method, "params": message.params},
                    }
                )
            else:
                raise ValueError(f"unhandled message type {message.__class__.__name__}")
        return (json_commands, lsp_id_map)

    def _verify_transcript(
        self, *, transcript: Transcript, lsp_id_map: "_LspIdMap"
    ) -> Iterable["_ErrorDescription"]:
        handled_entries = set()

        for message in self._messages:
            lsp_id = lsp_id_map[message]
            if isinstance(message, _RequestSpec):
                transcript_id = LspCommandProcessor._client_request_id(lsp_id)
                handled_entries.add(transcript_id)
                assert transcript_id in transcript, (
                    f"Expected message with ID {lsp_id!r} "
                    + f"to have an entry in the transcript "
                    + f"under key {transcript_id!r}, "
                    + f"but it was not found. Transcript: {transcript!r}"
                )
                entry = transcript[transcript_id]
                error_description = self._verify_request(
                    entry=entry, lsp_id=lsp_id, request=message
                )
                if error_description is not None:
                    yield error_description
            elif isinstance(message, _NotificationSpec):
                # Nothing needs to be done here, since we sent the notification
                # and don't expect a response.
                pass
            elif isinstance(message, (_WaitForRequestSpec, _WaitForNotificationSpec)):
                # Nothing needs to be done here -- if we failed to wait for the
                # message, an exception will have been thrown at the
                # `LspCommandProcessor` layer.
                pass
            else:
                raise ValueError(f"unhandled message type {message.__class__.__name__}")

        handled_entries |= set(self._find_ignored_transcript_ids(transcript))
        yield from self._flag_unhandled_notifications(
            handled_entries, transcript, lsp_id_map
        )

    def _verify_request(
        self, *, lsp_id: Json, entry: TranscriptEntry, request: "_RequestSpec"
    ) -> Optional["_ErrorDescription"]:
        actual_result = entry.received.get("result")
        actual_powered_by = entry.received.get("powered_by")
        if request.comment is not None:
            request_description = (
                f"Request with ID {lsp_id!r} (comment: {request.comment!r})"
            )
        else:
            request_description = f"Request with ID {lsp_id!r}"

        if actual_result != request.result:
            error_description = self._pretty_print_diff(
                actual=actual_result, expected=request.result
            )
            description = f"""\
{request_description} got an incorrect result:

{error_description}
    """
            request_context = self._get_context_for_traceback(request.traceback)
            context = f"""\
This was the associated request:

{request_context}"""
            remediation = self._describe_response_for_remediation(
                request=request, actual_response=entry.received
            )
            return _ErrorDescription(
                description=description, context=context, remediation=remediation
            )
        elif entry.received.get("powered_by") != request.powered_by:
            description = f"""\
{request_description} had an incorrect value for the `powered_by` field
(expected {request.powered_by!r}; got {actual_powered_by!r})
"""
            request_context = self._get_context_for_traceback(request.traceback)
            context = f"""\
This was the associated request:

{request_context}"""
            remediation = self._describe_response_for_remediation(
                request=request, actual_response=entry.received
            )
            return _ErrorDescription(
                description=description, context=context, remediation=remediation
            )

    def _get_context_for_traceback(self, traceback: _Traceback) -> str:
        # Find the first caller frame that isn't in this source file. The
        # assumption is that the first such frame is in the test code.
        caller_frame = next(frame for frame in traceback if frame.filename != __file__)
        source_filename = caller_frame.filename
        with open(source_filename) as f:
            source_text = f.read()

        (start_line_num, end_line_num) = self._find_line_range_for_function_call(
            file_contents=source_text, line_num=caller_frame.lineno
        )
        return self._pretty_print_file_context(
            file_path=source_filename,
            file_contents=source_text,
            start_line_num=start_line_num,
            end_line_num=end_line_num,
        )

    def _find_line_range_for_function_call(
        self, file_contents: str, line_num: int
    ) -> Tuple[int, int]:
        driver = lib2to3.pgen2.driver.Driver(
            grammar=lib2to3.pygram.python_grammar, convert=pytree.convert
        )
        tree = driver.parse_string(file_contents)

        function_call_pattern = lib2to3.patcomp.compile_pattern(  # pyre-ignore
            # For arithmetic precedence reasons, any regular, non-arithmetic
            # expression node is a 'power' node, since that has the most extreme
            # precedence in some respect. The 'trailer' denotes that it's
            # followed by an argument list.
            "power< any* trailer< '(' [any] ')' > >"
        )
        all_function_call_chains = [
            # For similar arithmetic precedence reasons, consecutive function
            # call and member access expressions appear to form one big n-ary
            # node, instead of a sequence of nested binary nodes.
            node
            for node in tree.pre_order()
            if function_call_pattern.match(node)
        ]
        all_function_calls = [
            # Flatten all elements of all chains into one list.
            child
            for chain in all_function_call_chains
            for child in chain.children
        ]
        function_calls_with_line_ranges = [
            (node, self._line_range_of_node(node)) for node in all_function_calls
        ]
        function_calls_containing_line = [
            (node, (max_line_num - min_line_num))
            for (node, (min_line_num, max_line_num)) in function_calls_with_line_ranges
            if min_line_num <= line_num <= max_line_num
        ]
        innermost_function_call = min(
            function_calls_containing_line, key=operator.itemgetter(1)
        )[0]
        (start_line_num, end_line_num) = self._line_range_of_node(
            innermost_function_call
        )
        start_line_num -= 1  # zero-index
        end_line_num -= 1  # zero-index
        return (start_line_num, end_line_num)

    def _line_range_of_node(self, node: pytree.Base) -> Tuple[int, int]:
        min_line_num = node.get_lineno()
        num_lines_in_node = str(node).count("\n")
        max_line_num = node.get_lineno() + num_lines_in_node
        return (min_line_num, max_line_num)

    def _pretty_print_file_context(
        self, file_path: str, file_contents: str, start_line_num: int, end_line_num: int
    ) -> str:
        source_lines = file_contents.splitlines(keepends=True)
        context_lines = source_lines[start_line_num : end_line_num + 1]
        vertical_bar = "\N{BOX DRAWINGS LIGHT VERTICAL}"
        context_lines = [
            # Include the line number in a gutter for display.
            f"{line_num:>5} {vertical_bar} {line_contents}"
            for line_num, line_contents in enumerate(
                context_lines, start=start_line_num + 1
            )
        ]
        file_context = "".join(context_lines)

        # The full path is likely not useful, since it includes any temporary
        # directories that Buck introduced.
        prefix = os.path.commonprefix([file_path, __file__])
        display_filename = file_path[len(prefix) :]
        return display_filename + "\n" + file_context

    def _describe_response_for_remediation(
        self, request: "_RequestSpec", actual_response: Json
    ) -> str:
        method = request.method
        params = request.params
        result = actual_response.get("result")
        powered_by = actual_response.get("powered_by")

        request_snippet = f"""\
    .request("""
        if request.comment is not None:
            request_snippet += f"""
        comment={request.comment!r},"""
        request_snippet += f"""
        method={method!r},
        params={params!r},
        result={result!r},"""
        if not request.wait:
            request_snippet += f"""
        wait=False,"""
        if request.powered_by is not None:
            request_snippet += f"""
        powered_by={powered_by!r},"""
        request_snippet += f"""
    )"""

        remediation = f"""\
1) If this was unexpected, then the language server is buggy and should be
fixed.

2) If this was expected, you can update your request with the following code to
make it match:

{request_snippet}
"""
        return remediation

    def _find_ignored_transcript_ids(self, transcript: Transcript) -> Iterable[str]:
        for transcript_id, entry in transcript.items():
            if (
                entry.received is not None
                and entry.received.get("method") in self._ignored_notification_methods
            ):
                yield transcript_id

    def _flag_unhandled_notifications(
        self,
        handled_entries: AbstractSet[str],
        transcript: Transcript,
        lsp_id_map: _LspIdMap,
    ) -> Iterable["_ErrorDescription"]:
        for transcript_id, entry in transcript.items():
            if transcript_id in handled_entries:
                continue

            received = entry.received
            if received is None:
                continue

            if entry.sent is not None:
                # We received a request and responded it it.
                continue

            method = received["method"]
            params = received["params"]
            payload = self._pretty_print_snippet(received)
            if "id" in received:
                description = f"""\
An unexpected request of type {method!r} was sent by the language server.
Here is the request payload:

{payload}
"""
                at_nocommit = "@" + "nocommit"
                remediation = f"""\
1) If this was unexpected, then the language server is buggy and should be
fixed.

2) To handle this request, add this directive to your test to wait for it and
respond to it before proceeding:

    .{self.wait_for_server_request.__name__}(
        method={method!r},
        params={params!r},
        result={{
            "{at_nocommit}": "fill in request data here",
        }},
    )
"""
            else:
                description = f"""\
An unexpected notification of type {method!r} was sent by the language server.
Here is the notification payload:

{payload}
"""
                remediation = f"""\
1) If this was unexpected, then the language server is buggy and should be
fixed.

2) If all notifications of type {method!r} should be ignored, add this directive
anywhere in your test:

    .{self.ignore_notifications.__name__}(method={method!r})

3) If this single instance of the notification was expected, add this directive
to your test to wait for it before proceeding:

    .{self.wait_for_notification.__name__}(
        method={method!r},
        params={params!r},
    )
"""

            previous_request = self._find_previous_request(
                transcript, lsp_id_map, current_id=transcript_id
            )
            if previous_request is not None:
                request_context = self._get_context_for_traceback(
                    previous_request.traceback
                )
            else:
                request_context = "<no previous request was found>"
            context = f"""\
This was the most recent request issued from the language client before it
received the notification:

{request_context}"""

            yield _ErrorDescription(
                description=description, context=context, remediation=remediation
            )

    def _find_previous_request(
        self, transcript: Transcript, lsp_id_map: _LspIdMap, current_id: str
    ) -> Optional["_RequestSpec"]:
        previous_transcript_entries = itertools.takewhile(
            lambda kv: kv[0] != current_id, transcript.items()
        )
        previous_request_entries = [
            entry.sent
            for _id, entry in previous_transcript_entries
            if entry.sent is not None and LspCommandProcessor._is_request(entry.sent)
        ]
        if previous_request_entries:
            previous_request_lsp_id = previous_request_entries[-1]["id"]
        else:
            return None

        [corresponding_request] = [
            request
            for request, lsp_id in lsp_id_map.items()
            if lsp_id == previous_request_lsp_id
        ]
        assert isinstance(
            corresponding_request, _RequestSpec
        ), "We should have identified a client-to-server request at this point"
        return corresponding_request

    def _pretty_print_snippet(self, obj: object) -> str:
        return textwrap.indent(pprint.pformat(obj), prefix="  ")

    def _pretty_print_diff(self, actual: object, expected: object) -> str:
        # Similar to the standard library's `unittest` module:
        # https://github.com/python/cpython/blob/35d9c37e271c35b87d64cc7422600e573f3ee244/Lib/unittest/case.py#L1147-L1149  # noqa B950
        return (
            "(+ is expected lines, - is actual lines)\n"
            + "\n".join(
                difflib.ndiff(
                    pprint.pformat(actual).splitlines(),
                    pprint.pformat(expected).splitlines(),
                )
            )
            + "\n"
        )


### Internal. ###


class _RequestSpec:
    __slots__ = [
        "method",
        "params",
        "result",
        "wait",
        "comment",
        "powered_by",
        "traceback",
    ]

    def __init__(
        self,
        *,
        method: str,
        params: Json,
        result: Json,
        wait: bool,
        comment: Optional[str],
        powered_by: Optional[str],
        traceback: _Traceback,
    ) -> None:
        self.method = method
        self.params = params
        self.result = result
        self.wait = wait
        self.comment = comment
        self.powered_by = powered_by
        self.traceback = traceback


class _NotificationSpec:
    __slots__ = ["method", "params", "comment"]

    def __init__(self, *, method: str, params: Json, comment: Optional[str]) -> None:
        self.method = method
        self.params = params
        self.comment = comment


class _WaitForRequestSpec:
    __slots__ = ["method", "params", "result", "comment"]

    def __init__(
        self, *, method: str, params: Json, result: Json, comment: Optional[str]
    ) -> None:
        self.method = method
        self.params = params
        self.result = result
        self.comment = comment


class _WaitForNotificationSpec:
    __slots__ = ["method", "params", "comment"]

    def __init__(self, *, method: str, params: Json, comment: Optional[str]) -> None:
        self.method = method
        self.params = params
        self.comment = comment


class _ErrorDescription:
    def __init__(self, description: str, context: str, remediation: str) -> None:
        self.description = description
        self.context = context
        self.remediation = remediation

    def __str__(self) -> str:
        result = f"""\
Description: {self.description}
"""
        if self.context is not None:
            result += f"""\
Context:
{self.context}
"""
        result += f"""\
Remediation:
{self.remediation}"""
        return result

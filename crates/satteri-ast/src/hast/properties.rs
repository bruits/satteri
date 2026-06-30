//! HAST property name → HTML/SVG attribute name mapping.
//!
//! Mirrors the attribute-name half of the
//! [`property-information`](https://github.com/wooorm/property-information)
//! package: every known HAST property resolves to its correctly-cased HTML or
//! SVG attribute, and unknown properties pass through unchanged.
//!
//! This does not yet carry the richer `Info` data (boolean / overloadedBoolean
//! / spaceSeparated / commaSeparated / booleanish / mustUseProperty); if we
//! need those for encoding-side decisions, the natural next step is a fuller
//! port, potentially as its own crate.

use std::borrow::Cow;

/// Convert a HAST (JS-style) property name to its serialized attribute name,
/// mirroring `property-information` / `hast-util-to-html`. `in_svg` selects
/// the SVG schema; otherwise HTML.
pub fn property_to_attribute(name: &str, in_svg: bool) -> Cow<'_, str> {
    // Every rewrite below is keyed on an uppercase ASCII letter: the namespace
    // prefixes only fire when an uppercase follows them, and every special-cased
    // and known property name is camelCase. Outside SVG, a name with no
    // uppercase letter is therefore always passed through unchanged, so skip the
    // whole ladder. This is the common case in HTML output (`href`, `src`,
    // `alt`, `id`, ...) and `property_to_attribute` runs once per attribute.
    if !in_svg && !name.bytes().any(|b| b.is_ascii_uppercase()) {
        return Cow::Borrowed(name);
    }

    if name == "xmlnsXLink" {
        return Cow::Borrowed("xmlns:xlink");
    }

    if let Some(rest) = strip_namespace_prefix(name, "xLink") {
        return Cow::Owned(format_namespace("xlink:", rest));
    }

    if let Some(rest) = strip_namespace_prefix(name, "xml") {
        return Cow::Owned(format_namespace("xml:", rest));
    }

    // ARIA is intentionally not kebab-cased between words: `ariaValueNow` →
    // `aria-valuenow`, not `aria-value-now`. ARIA spec convention; differs
    // from the data-* case below.
    if let Some(rest) = strip_namespace_prefix(name, "aria") {
        return Cow::Owned(format_namespace("aria-", rest));
    }

    // Schema lookup must beat the generic `data-*` fallback: `dataType` is a
    // real SVG attribute (→ `datatype`), not a custom `data-type`.
    if in_svg {
        if let Some(attr) = svg_attribute_for(name) {
            return Cow::Borrowed(attr);
        }
    }

    if let Some(rest) = strip_namespace_prefix(name, "data") {
        return Cow::Owned(format_data_attribute(rest));
    }

    if in_svg {
        return Cow::Borrowed(name);
    }

    match name {
        "className" => return Cow::Borrowed("class"),
        "htmlFor" => return Cow::Borrowed("for"),
        "httpEquiv" => return Cow::Borrowed("http-equiv"),
        "acceptCharset" => return Cow::Borrowed("accept-charset"),
        _ => {}
    }

    if is_known_lowercased_html_property(name) {
        return Cow::Owned(name.to_ascii_lowercase());
    }

    Cow::Borrowed(name)
}

/// Returns the suffix after `prefix` only when the next character is uppercase,
/// so bare words like `datatype` or `arial` don't get namespaced.
fn strip_namespace_prefix<'a>(name: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = name.strip_prefix(prefix)?;
    rest.starts_with(|c: char| c.is_ascii_uppercase())
        .then_some(rest)
}

fn format_namespace(prefix: &str, suffix: &str) -> String {
    let mut out = String::with_capacity(prefix.len() + suffix.len());
    out.push_str(prefix);
    for c in suffix.chars() {
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn format_data_attribute(suffix: &str) -> String {
    let mut out = String::with_capacity(4 + suffix.len() + 4);
    out.push_str("data");
    for c in suffix.chars() {
        if c.is_ascii_uppercase() {
            out.push('-');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Known HTML property names whose HTML attribute form is just the lowercased
/// property name. Mirrors the HTML schema in `property-information` (minus the
/// four special-cased entries handled above). Properties outside this list —
/// including custom/unknown props like `dangerouslySetInnerHTML` — are passed
/// through unchanged, matching `property-information`'s `find()` behavior.
///
/// The list is kept exhaustive rather than heuristic (e.g. "lowercase any
/// camelCase name") so that unknown/custom properties round-trip untouched.
fn is_known_lowercased_html_property(name: &str) -> bool {
    matches!(
        name,
        "accessKey"
            | "allowFullScreen"
            | "allowPaymentRequest"
            | "allowUserMedia"
            | "autoCapitalize"
            | "autoComplete"
            | "autoFocus"
            | "autoPlay"
            | "charSet"
            | "colSpan"
            | "contentEditable"
            | "controlsList"
            | "crossOrigin"
            | "dateTime"
            | "dirName"
            | "encType"
            | "enterKeyHint"
            | "fetchPriority"
            | "formAction"
            | "formEncType"
            | "formMethod"
            | "formNoValidate"
            | "formTarget"
            | "hrefLang"
            | "imageSizes"
            | "imageSrcSet"
            | "inputMode"
            | "isMap"
            | "itemId"
            | "itemProp"
            | "itemRef"
            | "itemScope"
            | "itemType"
            | "maxLength"
            | "minLength"
            | "noModule"
            | "noValidate"
            | "playsInline"
            | "popoverTarget"
            | "popoverTargetAction"
            | "readOnly"
            | "referrerPolicy"
            | "rowSpan"
            | "shadowRootClonable"
            | "shadowRootDelegatesFocus"
            | "shadowRootMode"
            | "spellCheck"
            | "srcDoc"
            | "srcLang"
            | "srcSet"
            | "tabIndex"
            | "typeMustMatch"
            | "useMap"
            | "writingSuggestions"
            | "onAbort"
            | "onAfterPrint"
            | "onAuxClick"
            | "onBeforeMatch"
            | "onBeforePrint"
            | "onBeforeToggle"
            | "onBeforeUnload"
            | "onBlur"
            | "onCancel"
            | "onCanPlay"
            | "onCanPlayThrough"
            | "onChange"
            | "onClick"
            | "onClose"
            | "onContextLost"
            | "onContextMenu"
            | "onContextRestored"
            | "onCopy"
            | "onCueChange"
            | "onCut"
            | "onDblClick"
            | "onDrag"
            | "onDragEnd"
            | "onDragEnter"
            | "onDragExit"
            | "onDragLeave"
            | "onDragOver"
            | "onDragStart"
            | "onDrop"
            | "onDurationChange"
            | "onEmptied"
            | "onEnded"
            | "onError"
            | "onFocus"
            | "onFormData"
            | "onHashChange"
            | "onInput"
            | "onInvalid"
            | "onKeyDown"
            | "onKeyPress"
            | "onKeyUp"
            | "onLanguageChange"
            | "onLoad"
            | "onLoadedData"
            | "onLoadedMetadata"
            | "onLoadEnd"
            | "onLoadStart"
            | "onMessage"
            | "onMessageError"
            | "onMouseDown"
            | "onMouseEnter"
            | "onMouseLeave"
            | "onMouseMove"
            | "onMouseOut"
            | "onMouseOver"
            | "onMouseUp"
            | "onOffline"
            | "onOnline"
            | "onPageHide"
            | "onPageShow"
            | "onPaste"
            | "onPause"
            | "onPlay"
            | "onPlaying"
            | "onPopState"
            | "onProgress"
            | "onRateChange"
            | "onRejectionHandled"
            | "onReset"
            | "onResize"
            | "onScroll"
            | "onScrollEnd"
            | "onSecurityPolicyViolation"
            | "onSeeked"
            | "onSeeking"
            | "onSelect"
            | "onSlotChange"
            | "onStalled"
            | "onStorage"
            | "onSubmit"
            | "onSuspend"
            | "onTimeUpdate"
            | "onToggle"
            | "onUnhandledRejection"
            | "onUnload"
            | "onVolumeChange"
            | "onWaiting"
            | "onWheel"
            | "aLink"
            | "bgColor"
            | "borderColor"
            | "bottomMargin"
            | "cellPadding"
            | "cellSpacing"
            | "charOff"
            | "classId"
            | "codeBase"
            | "codeType"
            | "frameBorder"
            | "hSpace"
            | "leftMargin"
            | "longDesc"
            | "lowSrc"
            | "marginHeight"
            | "marginWidth"
            | "noHref"
            | "noResize"
            | "noShade"
            | "noWrap"
            | "rightMargin"
            | "topMargin"
            | "vAlign"
            | "vLink"
            | "vSpace"
            | "valueType"
            | "allowTransparency"
            | "autoCorrect"
            | "autoSave"
            | "disablePictureInPicture"
            | "disableRemotePlayback"
    )
}

/// Explicit SVG property → attribute mappings
fn svg_attribute_for(name: &str) -> Option<&'static str> {
    Some(match name {
        "accentHeight" => "accent-height",
        "alignmentBaseline" => "alignment-baseline",
        "arabicForm" => "arabic-form",
        "baselineShift" => "baseline-shift",
        "capHeight" => "cap-height",
        "className" => "class",
        "clipPath" => "clip-path",
        "clipRule" => "clip-rule",
        "colorInterpolation" => "color-interpolation",
        "colorInterpolationFilters" => "color-interpolation-filters",
        "colorProfile" => "color-profile",
        "colorRendering" => "color-rendering",
        "crossOrigin" => "crossorigin",
        "dataType" => "datatype",
        "dominantBaseline" => "dominant-baseline",
        "enableBackground" => "enable-background",
        "fillOpacity" => "fill-opacity",
        "fillRule" => "fill-rule",
        "floodColor" => "flood-color",
        "floodOpacity" => "flood-opacity",
        "fontFamily" => "font-family",
        "fontSize" => "font-size",
        "fontSizeAdjust" => "font-size-adjust",
        "fontStretch" => "font-stretch",
        "fontStyle" => "font-style",
        "fontVariant" => "font-variant",
        "fontWeight" => "font-weight",
        "glyphName" => "glyph-name",
        "glyphOrientationHorizontal" => "glyph-orientation-horizontal",
        "glyphOrientationVertical" => "glyph-orientation-vertical",
        "hrefLang" => "hreflang",
        "horizAdvX" => "horiz-adv-x",
        "horizOriginX" => "horiz-origin-x",
        "horizOriginY" => "horiz-origin-y",
        "imageRendering" => "image-rendering",
        "letterSpacing" => "letter-spacing",
        "lightingColor" => "lighting-color",
        "markerEnd" => "marker-end",
        "markerMid" => "marker-mid",
        "markerStart" => "marker-start",
        "navDown" => "nav-down",
        "navDownLeft" => "nav-down-left",
        "navDownRight" => "nav-down-right",
        "navLeft" => "nav-left",
        "navNext" => "nav-next",
        "navPrev" => "nav-prev",
        "navRight" => "nav-right",
        "navUp" => "nav-up",
        "navUpLeft" => "nav-up-left",
        "navUpRight" => "nav-up-right",
        "onAbort" => "onabort",
        "onActivate" => "onactivate",
        "onAfterPrint" => "onafterprint",
        "onBeforePrint" => "onbeforeprint",
        "onBegin" => "onbegin",
        "onCancel" => "oncancel",
        "onCanPlay" => "oncanplay",
        "onCanPlayThrough" => "oncanplaythrough",
        "onChange" => "onchange",
        "onClick" => "onclick",
        "onClose" => "onclose",
        "onCopy" => "oncopy",
        "onCueChange" => "oncuechange",
        "onCut" => "oncut",
        "onDblClick" => "ondblclick",
        "onDrag" => "ondrag",
        "onDragEnd" => "ondragend",
        "onDragEnter" => "ondragenter",
        "onDragExit" => "ondragexit",
        "onDragLeave" => "ondragleave",
        "onDragOver" => "ondragover",
        "onDragStart" => "ondragstart",
        "onDrop" => "ondrop",
        "onDurationChange" => "ondurationchange",
        "onEmptied" => "onemptied",
        "onEnd" => "onend",
        "onEnded" => "onended",
        "onError" => "onerror",
        "onFocus" => "onfocus",
        "onFocusIn" => "onfocusin",
        "onFocusOut" => "onfocusout",
        "onHashChange" => "onhashchange",
        "onInput" => "oninput",
        "onInvalid" => "oninvalid",
        "onKeyDown" => "onkeydown",
        "onKeyPress" => "onkeypress",
        "onKeyUp" => "onkeyup",
        "onLoad" => "onload",
        "onLoadedData" => "onloadeddata",
        "onLoadedMetadata" => "onloadedmetadata",
        "onLoadStart" => "onloadstart",
        "onMessage" => "onmessage",
        "onMouseDown" => "onmousedown",
        "onMouseEnter" => "onmouseenter",
        "onMouseLeave" => "onmouseleave",
        "onMouseMove" => "onmousemove",
        "onMouseOut" => "onmouseout",
        "onMouseOver" => "onmouseover",
        "onMouseUp" => "onmouseup",
        "onMouseWheel" => "onmousewheel",
        "onOffline" => "onoffline",
        "onOnline" => "ononline",
        "onPageHide" => "onpagehide",
        "onPageShow" => "onpageshow",
        "onPaste" => "onpaste",
        "onPause" => "onpause",
        "onPlay" => "onplay",
        "onPlaying" => "onplaying",
        "onPopState" => "onpopstate",
        "onProgress" => "onprogress",
        "onRateChange" => "onratechange",
        "onRepeat" => "onrepeat",
        "onReset" => "onreset",
        "onResize" => "onresize",
        "onScroll" => "onscroll",
        "onSeeked" => "onseeked",
        "onSeeking" => "onseeking",
        "onSelect" => "onselect",
        "onShow" => "onshow",
        "onStalled" => "onstalled",
        "onStorage" => "onstorage",
        "onSubmit" => "onsubmit",
        "onSuspend" => "onsuspend",
        "onTimeUpdate" => "ontimeupdate",
        "onToggle" => "ontoggle",
        "onUnload" => "onunload",
        "onVolumeChange" => "onvolumechange",
        "onWaiting" => "onwaiting",
        "onZoom" => "onzoom",
        "overlinePosition" => "overline-position",
        "overlineThickness" => "overline-thickness",
        "paintOrder" => "paint-order",
        "panose1" => "panose-1",
        "pointerEvents" => "pointer-events",
        "referrerPolicy" => "referrerpolicy",
        "renderingIntent" => "rendering-intent",
        "shapeRendering" => "shape-rendering",
        "stopColor" => "stop-color",
        "stopOpacity" => "stop-opacity",
        "strikethroughPosition" => "strikethrough-position",
        "strikethroughThickness" => "strikethrough-thickness",
        "strokeDashArray" => "stroke-dasharray",
        "strokeDashOffset" => "stroke-dashoffset",
        "strokeLineCap" => "stroke-linecap",
        "strokeLineJoin" => "stroke-linejoin",
        "strokeMiterLimit" => "stroke-miterlimit",
        "strokeOpacity" => "stroke-opacity",
        "strokeWidth" => "stroke-width",
        "tabIndex" => "tabindex",
        "textAnchor" => "text-anchor",
        "textDecoration" => "text-decoration",
        "textRendering" => "text-rendering",
        "transformOrigin" => "transform-origin",
        "typeOf" => "typeof",
        "underlinePosition" => "underline-position",
        "underlineThickness" => "underline-thickness",
        "unicodeBidi" => "unicode-bidi",
        "unicodeRange" => "unicode-range",
        "unitsPerEm" => "units-per-em",
        "vAlphabetic" => "v-alphabetic",
        "vHanging" => "v-hanging",
        "vIdeographic" => "v-ideographic",
        "vMathematical" => "v-mathematical",
        "vectorEffect" => "vector-effect",
        "vertAdvY" => "vert-adv-y",
        "vertOriginX" => "vert-origin-x",
        "vertOriginY" => "vert-origin-y",
        "wordSpacing" => "word-spacing",
        "writingMode" => "writing-mode",
        "xHeight" => "x-height",
        "playbackOrder" => "playbackorder",
        "timelineBegin" => "timelinebegin",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::property_to_attribute;

    fn html(name: &str) -> std::borrow::Cow<'_, str> {
        property_to_attribute(name, false)
    }

    fn svg(name: &str) -> std::borrow::Cow<'_, str> {
        property_to_attribute(name, true)
    }

    #[test]
    fn html_special_cases() {
        assert_eq!(html("className"), "class");
        assert_eq!(html("htmlFor"), "for");
        assert_eq!(html("httpEquiv"), "http-equiv");
        assert_eq!(html("acceptCharset"), "accept-charset");
    }

    #[test]
    fn known_html_properties_are_lowercased() {
        assert_eq!(html("srcSet"), "srcset");
        assert_eq!(html("maxLength"), "maxlength");
        assert_eq!(html("minLength"), "minlength");
        assert_eq!(html("readOnly"), "readonly");
        assert_eq!(html("autoPlay"), "autoplay");
        assert_eq!(html("autoFocus"), "autofocus");
        assert_eq!(html("contentEditable"), "contenteditable");
        assert_eq!(html("tabIndex"), "tabindex");
        assert_eq!(html("colSpan"), "colspan");
        assert_eq!(html("rowSpan"), "rowspan");
        assert_eq!(html("crossOrigin"), "crossorigin");
        assert_eq!(html("dateTime"), "datetime");
        assert_eq!(html("charSet"), "charset");
        assert_eq!(html("noValidate"), "novalidate");
        assert_eq!(html("referrerPolicy"), "referrerpolicy");
        assert_eq!(html("inputMode"), "inputmode");
        assert_eq!(html("enterKeyHint"), "enterkeyhint");
        assert_eq!(html("spellCheck"), "spellcheck");
        assert_eq!(html("accessKey"), "accesskey");
        assert_eq!(html("itemProp"), "itemprop");
        assert_eq!(html("imageSrcSet"), "imagesrcset");
        assert_eq!(html("formNoValidate"), "formnovalidate");
    }

    #[test]
    fn event_handlers_are_lowercased() {
        assert_eq!(html("onClick"), "onclick");
        assert_eq!(html("onKeyDown"), "onkeydown");
        assert_eq!(html("onMouseOver"), "onmouseover");
        assert_eq!(html("onCanPlayThrough"), "oncanplaythrough");
    }

    #[test]
    fn legacy_properties_are_lowercased() {
        assert_eq!(html("bgColor"), "bgcolor");
        assert_eq!(html("cellPadding"), "cellpadding");
        assert_eq!(html("vAlign"), "valign");
        assert_eq!(html("longDesc"), "longdesc");
    }

    #[test]
    fn aria_lowercases_suffix_without_inner_hyphens() {
        assert_eq!(html("ariaHidden"), "aria-hidden");
        assert_eq!(html("ariaLive"), "aria-live");
        // ARIA attributes do NOT get inner hyphens between words.
        assert_eq!(html("ariaValueNow"), "aria-valuenow");
        assert_eq!(html("ariaActiveDescendant"), "aria-activedescendant");
        // ARIA works the same in SVG context.
        assert_eq!(svg("ariaHidden"), "aria-hidden");
        assert_eq!(svg("ariaValueNow"), "aria-valuenow");
    }

    #[test]
    fn data_kebab_cases_suffix() {
        assert_eq!(html("dataLanguage"), "data-language");
        assert_eq!(html("dataFooBar"), "data-foo-bar");
        // data-* works the same in SVG context.
        assert_eq!(svg("dataLanguage"), "data-language");
    }

    #[test]
    fn xlink_namespaces_lowercased_suffix() {
        assert_eq!(html("xLinkHref"), "xlink:href");
        assert_eq!(html("xLinkActuate"), "xlink:actuate");
        assert_eq!(html("xLinkArcRole"), "xlink:arcrole");
        assert_eq!(html("xLinkType"), "xlink:type");
        // xlink works the same in SVG context (it's where it actually belongs).
        assert_eq!(svg("xLinkHref"), "xlink:href");
    }

    #[test]
    fn xml_namespaces_lowercased_suffix() {
        assert_eq!(html("xmlLang"), "xml:lang");
        assert_eq!(html("xmlBase"), "xml:base");
        assert_eq!(html("xmlSpace"), "xml:space");
        assert_eq!(svg("xmlLang"), "xml:lang");
    }

    #[test]
    fn xmlns_special_cases() {
        assert_eq!(html("xmlnsXLink"), "xmlns:xlink");
        assert_eq!(svg("xmlnsXLink"), "xmlns:xlink");
    }

    #[test]
    fn unknown_properties_pass_through() {
        assert_eq!(html("foo"), "foo");
        assert_eq!(html("my-custom"), "my-custom");
        // Property that does not start with an uppercase after the prefix is unchanged.
        assert_eq!(html("datatype"), "datatype");
        assert_eq!(html("arial"), "arial");
        // Custom/React-style properties unknown to property-information pass through.
        assert_eq!(html("dangerouslySetInnerHTML"), "dangerouslySetInnerHTML");
        assert_eq!(html("customProp"), "customProp");
    }

    #[test]
    fn svg_kebab_cased_attributes() {
        assert_eq!(svg("fillRule"), "fill-rule");
        assert_eq!(svg("clipRule"), "clip-rule");
        assert_eq!(svg("strokeWidth"), "stroke-width");
        assert_eq!(svg("strokeLineCap"), "stroke-linecap");
        assert_eq!(svg("strokeLineJoin"), "stroke-linejoin");
        assert_eq!(svg("strokeDashArray"), "stroke-dasharray");
        assert_eq!(svg("strokeDashOffset"), "stroke-dashoffset");
        assert_eq!(svg("alignmentBaseline"), "alignment-baseline");
        assert_eq!(svg("dominantBaseline"), "dominant-baseline");
        assert_eq!(svg("textAnchor"), "text-anchor");
        assert_eq!(svg("transformOrigin"), "transform-origin");
        assert_eq!(svg("vectorEffect"), "vector-effect");
        assert_eq!(svg("xHeight"), "x-height");
        assert_eq!(svg("panose1"), "panose-1");
    }

    #[test]
    fn svg_lowercased_attributes() {
        assert_eq!(svg("crossOrigin"), "crossorigin");
        assert_eq!(svg("hrefLang"), "hreflang");
        assert_eq!(svg("referrerPolicy"), "referrerpolicy");
        assert_eq!(svg("tabIndex"), "tabindex");
        assert_eq!(svg("typeOf"), "typeof");
        assert_eq!(svg("dataType"), "datatype");
        assert_eq!(svg("playbackOrder"), "playbackorder");
        assert_eq!(svg("timelineBegin"), "timelinebegin");
        assert_eq!(svg("onClick"), "onclick");
    }

    #[test]
    fn svg_case_preserved_attributes() {
        // These appear in SVG's `properties` map but NOT in the `attributes`
        // map, so they're case-preserved.
        assert_eq!(svg("viewBox"), "viewBox");
        assert_eq!(svg("preserveAspectRatio"), "preserveAspectRatio");
        assert_eq!(svg("gradientUnits"), "gradientUnits");
        assert_eq!(svg("gradientTransform"), "gradientTransform");
        assert_eq!(svg("patternUnits"), "patternUnits");
        assert_eq!(svg("patternTransform"), "patternTransform");
        assert_eq!(svg("clipPathUnits"), "clipPathUnits");
        assert_eq!(svg("maskUnits"), "maskUnits");
        assert_eq!(svg("maskContentUnits"), "maskContentUnits");
        assert_eq!(svg("markerUnits"), "markerUnits");
        assert_eq!(svg("primitiveUnits"), "primitiveUnits");
        assert_eq!(svg("filterUnits"), "filterUnits");
        assert_eq!(svg("baseFrequency"), "baseFrequency");
        assert_eq!(svg("numOctaves"), "numOctaves");
        assert_eq!(svg("stdDeviation"), "stdDeviation");
        assert_eq!(svg("attributeName"), "attributeName");
        assert_eq!(svg("attributeType"), "attributeType");
        assert_eq!(svg("repeatCount"), "repeatCount");
        assert_eq!(svg("keyTimes"), "keyTimes");
        assert_eq!(svg("keySplines"), "keySplines");
        assert_eq!(svg("keyPoints"), "keyPoints");
        assert_eq!(svg("xChannelSelector"), "xChannelSelector");
        assert_eq!(svg("yChannelSelector"), "yChannelSelector");
        assert_eq!(svg("zoomAndPan"), "zoomAndPan");
        // Already-lowercase SVG attrs (case-preserved trivially).
        assert_eq!(svg("width"), "width");
        assert_eq!(svg("height"), "height");
        assert_eq!(svg("fill"), "fill");
        assert_eq!(svg("d"), "d");
    }

    #[test]
    fn svg_unknown_passes_through() {
        // Custom SVG-namespace attrs we don't know about: pass through.
        assert_eq!(svg("customAttr"), "customAttr");
        assert_eq!(svg("foo"), "foo");
    }

    #[test]
    fn html_only_names_in_svg_context_pass_through() {
        // `htmlFor` / `httpEquiv` / `acceptCharset` are HTML-only; in SVG the
        // schema doesn't know them, so they pass through unchanged.
        assert_eq!(svg("htmlFor"), "htmlFor");
        assert_eq!(svg("httpEquiv"), "httpEquiv");
        assert_eq!(svg("acceptCharset"), "acceptCharset");
    }

    #[test]
    fn html_only_lowercased_in_svg_context_pass_through() {
        // SVG does not lowercase arbitrary HTML props; only those in its
        // explicit table get rewritten.
        assert_eq!(svg("srcSet"), "srcSet");
        assert_eq!(svg("maxLength"), "maxLength");
        assert_eq!(svg("readOnly"), "readOnly");
        assert_eq!(svg("contentEditable"), "contentEditable");
    }
}

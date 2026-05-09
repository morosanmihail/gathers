import { useState } from "react";
import { useSearchParams } from "react-router-dom";

export default function useCardSearch({ stringFields, arrayFields = [], startSearch = false, defaults = {} }) {
  const [searchParams, setSearchParams] = useSearchParams();

  const initialOptions = {
    ...Object.fromEntries(stringFields.map((f) => [f, searchParams.get(f) ?? (defaults[f] ?? "")])),
    ...Object.fromEntries(arrayFields.map((f) => [f, searchParams.getAll(f)])),
  };

  const [pageNumber, setPageNumber] = useState(parseInt(searchParams.get("page") ?? "1"));
  const [searchOptions, setSearchOptions] = useState(initialOptions);
  const [cards, setCards] = useState([]);
  const [loading, setLoading] = useState(false);

  const hasParams = Object.entries(initialOptions).some(([f, v]) => {
    if (Array.isArray(v)) return v.length > 0;
    const defaultVal = defaults[f] ?? "";
    return v !== "" && v !== defaultVal && searchParams.has(f);
  });
  const [shouldSearch, setShouldSearch] = useState(startSearch || hasParams);

  const handleSearchInput = (event, field) => {
    const newState = { ...searchOptions, [field]: event.target.value };
    setSearchOptions(newState);
    setSearchParams({ ...newState, page: "1" });
  };

  const handleArrayInput = (field, event) => {
    const filtered = searchOptions[field].filter((v) => v !== event.target.value);
    const newState = {
      ...searchOptions,
      [field]: event.target.checked ? [...filtered, event.target.value] : filtered,
    };
    setSearchOptions(newState);
    setSearchParams({ ...newState, page: "1" });
  };

  const handlePageChange = (event) => {
    const newPage = parseInt(event.selected) + 1;
    setShouldSearch(true);
    setPageNumber(newPage);
    setSearchParams({ ...searchOptions, page: String(newPage) });
  };

  const handleMultiInput = (updates, { search = false } = {}) => {
    const newState = { ...searchOptions, ...updates };
    setSearchOptions(newState);
    setSearchParams({ ...newState, page: "1" });
    if (search) {
      setPageNumber(1);
      setShouldSearch(true);
    }
  };

  const triggerSearch = () => {
    setPageNumber(1);
    setShouldSearch(true);
    setSearchParams({ ...searchOptions, page: "1" });
  };

  return {
    cards, setCards,
    loading, setLoading,
    pageNumber,
    shouldSearch, setShouldSearch,
    searchOptions,
    handleSearchInput,
    handleArrayInput,
    handleMultiInput,
    handlePageChange,
    triggerSearch,
  };
}
